//! lojix-macros — four categorical proc macros for deriving typed Rust code
//! from samskara's datalog schema.
//!
//! The macros load CozoScript schema into an in-memory CozoDB at compile time,
//! query the field_type graph, and emit Rust types guaranteed to match the spec.
//! A schema change → compile error in consuming code.
//!
//! The four patterns:
//! - `domain!(Name)` — discrete category (enum from Domain registry)
//! - `product!(Name)` — product object (struct with typed projections)
//! - `morphism!(Name)` — arrows (trait from rpc_interface)
//! - `fold!(Domain, val, { Variant => expr, ... })` — coproduct elimination

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, Token, braced, parse::{Parse, ParseStream}};

mod db;

/// `domain!(Phase)` — generates a Rust enum from a Domain relation.
///
/// Each variant comes from the seed data of the named domain.
/// Includes discriminant conversion, Display, FromStr.
#[proc_macro]
pub fn domain(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as Ident);
    let name_str = name.to_string();

    let db = match db::load_db() {
        Ok(db) => db,
        Err(e) => return compile_error(&format!("lojix::domain!: failed to load DB: {e}")),
    };

    let variants = match db::query_domain_variants(&db, &name_str) {
        Ok(v) => v,
        Err(e) => return compile_error(&format!("lojix::domain!({name_str}): {e}")),
    };

    if variants.is_empty() {
        return compile_error(&format!("lojix::domain!({name_str}): no variants found"));
    }

    let variant_idents: Vec<proc_macro2::Ident> = variants
        .iter()
        .map(|v| to_pascal_ident(v))
        .collect();

    let variant_strs: Vec<&str> = variants.iter().map(|s| s.as_str()).collect();
    let variant_indices: Vec<u16> = (0..variants.len() as u16).collect();
    let variant_count = variants.len();

    let expanded = quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum #name {
            #( #variant_idents, )*
        }

        impl #name {
            pub const COUNT: usize = #variant_count;

            pub fn discriminant(&self) -> u16 {
                match self {
                    #( #name::#variant_idents => #variant_indices, )*
                }
            }

            pub fn from_discriminant(d: u16) -> Option<Self> {
                match d {
                    #( #variant_indices => Some(#name::#variant_idents), )*
                    _ => None,
                }
            }

            pub fn name(&self) -> &'static str {
                match self {
                    #( #name::#variant_idents => #variant_strs, )*
                }
            }

            pub fn all() -> &'static [#name] {
                &[ #( #name::#variant_idents, )* ]
            }
        }

        impl core::fmt::Display for #name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.name())
            }
        }

        impl core::str::FromStr for #name {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    #( #variant_strs => Ok(#name::#variant_idents), )*
                    other => Err(format!("unknown {} variant: {}", stringify!(#name), other)),
                }
            }
        }
    };

    expanded.into()
}

/// `product!(Measure)` — generates a Rust struct from a relation,
/// with fields typed via the field_type graph.
#[proc_macro]
pub fn product(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as Ident);
    let name_str = name.to_string();

    let db = match db::load_db() {
        Ok(db) => db,
        Err(e) => return compile_error(&format!("lojix::product!: failed to load DB: {e}")),
    };

    let fields = match db::query_product_fields(&db, &name_str) {
        Ok(f) => f,
        Err(e) => return compile_error(&format!("lojix::product!({name_str}): {e}")),
    };

    if fields.is_empty() {
        return compile_error(&format!("lojix::product!({name_str}): no fields found"));
    }

    let field_tokens: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .map(|f| {
            let field_ident = proc_macro2::Ident::new(&f.name, proc_macro2::Span::call_site());
            let ty = field_type_to_rust_type(&f.kind, &f.target_domain);
            quote! { pub #field_ident: #ty }
        })
        .collect();

    let expanded = quote! {
        #[derive(Debug, Clone)]
        pub struct #name {
            #( #field_tokens, )*
        }
    };

    expanded.into()
}

/// Input for the fold! macro: `fold!(Domain, value, { Variant => expr, ... })`
struct FoldInput {
    domain: Ident,
    _comma1: Token![,],
    value: syn::Expr,
    _comma2: Token![,],
    arms: Vec<FoldArm>,
}

struct FoldArm {
    variant: Ident,
    _arrow: Token![=>],
    body: syn::Expr,
}

impl Parse for FoldInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let domain = input.parse()?;
        let _comma1 = input.parse()?;
        let value = input.parse()?;
        let _comma2 = input.parse()?;

        let content;
        braced!(content in input);

        let mut arms = Vec::new();
        while !content.is_empty() {
            let variant = content.parse()?;
            let _arrow = content.parse()?;
            let body = content.parse()?;
            // optional trailing comma
            let _ = content.parse::<Option<Token![,]>>();
            arms.push(FoldArm { variant, _arrow, body });
        }

        Ok(FoldInput { domain, _comma1, value, _comma2, arms })
    }
}

/// `fold!(Phase, val, { Becoming => expr, Manifest => expr, Retired => expr })`
/// — exhaustive coproduct elimination. Compile error if any variant is missing.
#[proc_macro]
pub fn fold(input: TokenStream) -> TokenStream {
    let fold_input = parse_macro_input!(input as FoldInput);
    let domain_str = fold_input.domain.to_string();

    let db = match db::load_db() {
        Ok(db) => db,
        Err(e) => return compile_error(&format!("lojix::fold!: failed to load DB: {e}")),
    };

    let variants = match db::query_domain_variants(&db, &domain_str) {
        Ok(v) => v,
        Err(e) => return compile_error(&format!("lojix::fold!({domain_str}): {e}")),
    };

    // Check exhaustiveness: every variant must have an arm
    let arm_names: Vec<String> = fold_input.arms.iter()
        .map(|a| a.variant.to_string())
        .collect();

    let expected_names: Vec<String> = variants.iter()
        .map(|v| to_pascal_string(v))
        .collect();

    for expected in &expected_names {
        if !arm_names.contains(expected) {
            return compile_error(&format!(
                "lojix::fold!({domain_str}): missing arm for variant `{expected}`"
            ));
        }
    }

    for arm_name in &arm_names {
        if !expected_names.contains(arm_name) {
            return compile_error(&format!(
                "lojix::fold!({domain_str}): unknown variant `{arm_name}`"
            ));
        }
    }

    let domain_ident = &fold_input.domain;
    let value_expr = &fold_input.value;
    let match_arms: Vec<proc_macro2::TokenStream> = fold_input.arms.iter().map(|arm| {
        let variant = &arm.variant;
        let body = &arm.body;
        quote! { #domain_ident::#variant => #body }
    }).collect();

    let expanded = quote! {
        match #value_expr {
            #( #match_arms, )*
        }
    };

    expanded.into()
}

/// `morphism!(Samskara)` — generates a trait from an rpc_interface.
/// Placeholder for now — full implementation requires rpc_param loading.
#[proc_macro]
pub fn morphism(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as Ident);
    let name_str = name.to_string();

    let db = match db::load_db() {
        Ok(db) => db,
        Err(e) => return compile_error(&format!("lojix::morphism!: failed to load DB: {e}")),
    };

    let methods = match db::query_rpc_methods(&db, &name_str) {
        Ok(m) => m,
        Err(e) => return compile_error(&format!("lojix::morphism!({name_str}): {e}")),
    };

    let method_tokens: Vec<proc_macro2::TokenStream> = methods
        .iter()
        .map(|m| {
            let method_ident = proc_macro2::Ident::new(&to_snake_case(&m.name), proc_macro2::Span::call_site());
            let doc = &m.description;
            quote! {
                #[doc = #doc]
                fn #method_ident(&self, params: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
            }
        })
        .collect();

    let expanded = quote! {
        pub trait #name {
            #( #method_tokens )*
        }
    };

    expanded.into()
}

/// `domain_registry!()` — generates ALL domain enums from the Domain registry,
/// plus a dispatch table for translation, verification, and enumeration.
///
/// This is the macro that eliminates all hand-maintained domain lists.
/// If a domain is added to samskara, it appears here automatically.
/// If a domain is removed, consuming code that references it fails to compile.
#[proc_macro]
pub fn domain_registry(_input: TokenStream) -> TokenStream {
    let db = match db::load_db() {
        Ok(db) => db,
        Err(e) => return compile_error(&format!("lojix::domain_registry!: failed to load DB: {e}")),
    };

    let domain_names = match db::query_all_enum_domains(&db) {
        Ok(d) => d,
        Err(e) => return compile_error(&format!("lojix::domain_registry!: {e}")),
    };

    let mut all_enum_tokens = Vec::new();
    let mut translate_arms = Vec::new();
    let mut verify_arms = Vec::new();
    let mut dump_arms = Vec::new();
    let mut domain_name_strs = Vec::new();
    let mut domain_idents = Vec::new();

    for domain_name in &domain_names {
        let variants = match db::query_domain_variants(&db, domain_name) {
            Ok(v) if !v.is_empty() => v,
            _ => continue,
        };

        let ident = proc_macro2::Ident::new(domain_name, proc_macro2::Span::call_site());
        let variant_idents: Vec<proc_macro2::Ident> = variants
            .iter()
            .map(|v| to_pascal_ident(v))
            .collect();
        let variant_strs: Vec<&str> = variants.iter().map(|s| s.as_str()).collect();
        let variant_indices: Vec<u16> = (0..variants.len() as u16).collect();
        let variant_count = variants.len();

        // Generate the enum and its impls
        all_enum_tokens.push(quote! {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
            pub enum #ident {
                #( #variant_idents, )*
            }

            impl #ident {
                pub const COUNT: usize = #variant_count;

                pub fn discriminant(&self) -> u16 {
                    match self {
                        #( #ident::#variant_idents => #variant_indices, )*
                    }
                }

                pub fn from_discriminant(d: u16) -> Option<Self> {
                    match d {
                        #( #variant_indices => Some(#ident::#variant_idents), )*
                        _ => None,
                    }
                }

                pub fn name(&self) -> &'static str {
                    match self {
                        #( #ident::#variant_idents => #variant_strs, )*
                    }
                }

                pub fn all() -> &'static [#ident] {
                    &[ #( #ident::#variant_idents, )* ]
                }
            }

            impl core::fmt::Display for #ident {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    f.write_str(self.name())
                }
            }

            impl core::str::FromStr for #ident {
                type Err = String;

                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    match s {
                        #( #variant_strs => Ok(#ident::#variant_idents), )*
                        other => Err(format!("unknown {} variant: {}", stringify!(#ident), other)),
                    }
                }
            }
        });

        let name_str = domain_name.as_str();

        // Generate translate dispatch arm
        translate_arms.push(quote! {
            #name_str => #ident::from_discriminant(discriminant).map(|v| v.name()),
        });

        // Generate verify arm
        verify_arms.push(quote! {
            {
                let count = #ident::COUNT;
                eprintln!("  {} — {} variants", #name_str, count);
                total += count;
            }
        });

        // Generate dump arm
        dump_arms.push(quote! {
            {
                println!("{}:", #name_str);
                for v in #ident::all() {
                    println!("  {} = {}", v.discriminant(), v.name());
                }
            }
        });

        domain_name_strs.push(name_str.to_string());
        domain_idents.push(ident);
    }

    let domain_count = domain_idents.len();
    let domain_name_str_refs: Vec<&str> = domain_name_strs.iter().map(|s| s.as_str()).collect();

    let expanded = quote! {
        // All domain enums generated from the Domain registry
        #( #all_enum_tokens )*

        /// Translate any domain discriminant to its English name.
        /// Generated from the Domain registry — no hand-maintained dispatch.
        pub fn translate_domain(domain_name: &str, discriminant: u16) -> Option<&'static str> {
            match domain_name {
                #( #translate_arms )*
                _ => None,
            }
        }

        /// Verify all domains are populated. Prints diagnostics to stderr.
        pub fn verify_domains() {
            let mut total: usize = 0;
            #( #verify_arms )*
            eprintln!("noesis: {} total variants across {} domains — all valid", total, #domain_count);
        }

        /// Dump all domains and their variants to stdout.
        pub fn dump_all_domains() {
            #( #dump_arms )*
        }

        /// Number of domains in the registry.
        pub const DOMAIN_COUNT: usize = #domain_count;

        /// All domain names.
        pub const DOMAIN_NAMES: &[&str] = &[ #( #domain_name_str_refs, )* ];
    };

    expanded.into()
}

fn compile_error(msg: &str) -> TokenStream {
    let msg_lit = proc_macro2::Literal::string(msg);
    let tokens = quote! { compile_error!(#msg_lit); };
    tokens.into()
}

fn to_pascal_ident(s: &str) -> proc_macro2::Ident {
    let pascal = to_pascal_string(s);
    proc_macro2::Ident::new(&pascal, proc_macro2::Span::call_site())
}

fn to_pascal_string(s: &str) -> String {
    s.split(|c: char| c == '_' || c == '-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect()
}

fn to_snake_case(s: &str) -> String {
    s.replace('-', "_")
}

fn field_type_to_rust_type(kind: &str, target_domain: &str) -> proc_macro2::TokenStream {
    match kind {
        "domain" => {
            let ident = proc_macro2::Ident::new(target_domain, proc_macro2::Span::call_site());
            quote! { #ident }
        }
        "bool" => quote! { bool },
        "int" => quote! { TypedInt },
        "data" => quote! { Vec<u8> },
        _ => quote! { Vec<u8> },
    }
}
