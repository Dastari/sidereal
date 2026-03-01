use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Error, Expr, ExprArray, ExprLit, Ident, Item, Lit, LitBool, LitStr, Result, Token,
    parse_macro_input,
};

struct SiderealComponentArgs {
    kind: LitStr,
    persist: LitBool,
    replicate: LitBool,
    predict: LitBool,
    visibility: Option<ExprArray>,
}

impl Parse for SiderealComponentArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut kind = None;
        let mut persist = None;
        let mut replicate = None;
        let mut predict = None;
        let mut visibility = None;

        let entries = Punctuated::<MetaArg, Token![,]>::parse_terminated(input)?;
        for entry in entries {
            match entry {
                MetaArg::Kind(v) => kind = Some(v),
                MetaArg::Persist(v) => persist = Some(v),
                MetaArg::Replicate(v) => replicate = Some(v),
                MetaArg::Predict(v) => predict = Some(v),
                MetaArg::Visibility(v) => visibility = Some(v),
            }
        }

        Ok(Self {
            kind: kind
                .ok_or_else(|| Error::new(input.span(), "missing required argument: kind"))?,
            persist: persist.unwrap_or(LitBool::new(true, input.span())),
            replicate: replicate.unwrap_or(LitBool::new(true, input.span())),
            predict: predict.unwrap_or(LitBool::new(false, input.span())),
            visibility,
        })
    }
}

enum MetaArg {
    Kind(LitStr),
    Persist(LitBool),
    Replicate(LitBool),
    Predict(LitBool),
    Visibility(ExprArray),
}

impl Parse for MetaArg {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        match key.to_string().as_str() {
            "kind" => {
                let lit: LitStr = input.parse()?;
                Ok(Self::Kind(lit))
            }
            "persist" => {
                let lit: LitBool = input.parse()?;
                Ok(Self::Persist(lit))
            }
            "replicate" => {
                let lit: LitBool = input.parse()?;
                Ok(Self::Replicate(lit))
            }
            "predict" => {
                let lit: LitBool = input.parse()?;
                Ok(Self::Predict(lit))
            }
            "visibility" => {
                let expr: Expr = input.parse()?;
                match expr {
                    Expr::Array(arr) => Ok(Self::Visibility(arr)),
                    _ => Err(Error::new(
                        expr.span(),
                        "visibility must be an array, e.g. [OwnerOnly, Public]",
                    )),
                }
            }
            _ => Err(Error::new(
                key.span(),
                "unknown sidereal_component argument",
            )),
        }
    }
}

#[proc_macro_attribute]
pub fn sidereal_component(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as SiderealComponentArgs);
    let item_ast = parse_macro_input!(item as Item);

    let item_ident = match &item_ast {
        Item::Struct(s) => s.ident.clone(),
        Item::Enum(e) => e.ident.clone(),
        _ => {
            return Error::new_spanned(
                item_ast,
                "#[sidereal_component] supports structs/enums only",
            )
            .to_compile_error()
            .into();
        }
    };

    let kind = args.kind;
    let persist = args.persist.value;
    let replicate = args.replicate.value;
    let predict = args.predict.value;
    let register_fn_ident = format_ident!(
        "__sidereal_register_reflect_{}",
        item_ident.to_string().to_lowercase()
    );
    let register_lightyear_fn_ident = format_ident!(
        "__sidereal_register_lightyear_{}",
        item_ident.to_string().to_lowercase()
    );
    let type_path_fn_ident = format_ident!(
        "__sidereal_type_path_{}",
        item_ident.to_string().to_lowercase()
    );

    let visibility_items = if let Some(arr) = args.visibility {
        let mut scopes = Vec::new();
        for expr in arr.elems {
            match expr {
                Expr::Path(path) => {
                    let seg = path.path.segments.last().map(|s| s.ident.clone());
                    if let Some(ident) = seg {
                        scopes.push(quote! { crate::component_meta::VisibilityScope::#ident });
                    }
                }
                Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) => {
                    let ident = Ident::new(&s.value(), s.span());
                    scopes.push(quote! { crate::component_meta::VisibilityScope::#ident });
                }
                other => {
                    return Error::new(other.span(), "invalid visibility entry")
                        .to_compile_error()
                        .into();
                }
            }
        }
        if scopes.is_empty() {
            quote! { &[crate::component_meta::VisibilityScope::OwnerOnly] }
        } else {
            quote! { &[#(#scopes),*] }
        }
    } else {
        quote! { &[crate::component_meta::VisibilityScope::OwnerOnly] }
    };

    let lightyear_body = if replicate && predict {
        quote! {
            #[cfg(feature = "lightyear")]
            {
                use lightyear::prelude::AppComponentExt;
                use lightyear::prediction::prelude::PredictionRegistrationExt;
                app.register_component::<#item_ident>().add_prediction();
            }
        }
    } else if replicate {
        quote! {
            #[cfg(feature = "lightyear")]
            {
                use lightyear::prelude::AppComponentExt;
                app.register_component::<#item_ident>();
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #item_ast

        impl crate::component_meta::SiderealComponentMetadata for #item_ident {
            const META: crate::component_meta::SiderealComponentMeta = crate::component_meta::SiderealComponentMeta {
                kind: #kind,
                persist: #persist,
                replicate: #replicate,
                predict: #predict,
                visibility: #visibility_items,
            };
        }

        fn #register_fn_ident(app: &mut bevy::prelude::App) {
            app.register_type::<#item_ident>();
        }

        #[allow(unused_variables)]
        fn #register_lightyear_fn_ident(app: &mut bevy::prelude::App) {
            #lightyear_body
        }

        fn #type_path_fn_ident() -> &'static str {
            std::any::type_name::<#item_ident>()
        }

        inventory::submit! {
            crate::component_meta::SiderealComponentRegistration {
                register_reflect: #register_fn_ident,
                register_lightyear: #register_lightyear_fn_ident,
                type_path: #type_path_fn_ident,
                meta: <#item_ident as crate::component_meta::SiderealComponentMetadata>::META,
            }
        }
    };

    expanded.into()
}
