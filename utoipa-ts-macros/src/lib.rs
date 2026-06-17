use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Expr, Ident, ItemFn, LitStr, Result, Token, Type, parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Registers a [`utoipa`] endpoint for TypeScript API generation
///
/// This macro forwards all arguments to [`utoipa::path`] and additionally records
/// supported endpoint metadata for [`utoipa_ts::export!`]
///
/// [`utoipa`]: https://docs.rs/utoipa/latest/utoipa
/// [`utoipa::path`]: https://docs.rs/utoipa/latest/utoipa/attr.path.html
/// [`utoipa_ts::export!`]: https://docs.rs/utoipa-ts/latest/utoipa_ts/macro.export.html
///
/// ## Supported metadata
///
/// - HTTP method, e.g. `get`, `post`, `put`
/// - `path = "/..."`
/// - `params(...)`
/// - `request_body = Type`
/// - `request_body(content = Type, ...)`
/// - `responses((status = 200, body = Type), ...)`
#[proc_macro_attribute]
pub fn path(args: TokenStream, item: TokenStream) -> TokenStream {
    let original_args = TokenStream2::from(args.clone());
    let args = parse_macro_input!(args as PathArgs);
    let input = parse_macro_input!(item as ItemFn);

    expand_path(original_args, args, input).into()
}

fn expand_path(original_args: TokenStream2, args: PathArgs, input: ItemFn) -> TokenStream2 {
    let fn_name = &input.sig.ident;
    let render_name = format_ident!("__utoipa_ts_render_{}", fn_name);
    let method = args.method.unwrap_or_else(|| "UNKNOWN".to_owned());
    let path = args.path.unwrap_or_else(|| "/".to_owned());

    let params = args.params.iter().map(|param| {
        let name = &param.name;
        let ty = &param.ty;
        quote! {
            endpoint.param::<#ty>(#name);
        }
    });

    let param_sets = args.param_sets.iter().map(|ty| {
        quote! {
            endpoint.params::<#ty>();
        }
    });

    let request_body = args.request_body.iter().map(|ty| {
        quote! {
            endpoint.request_body::<#ty>();
        }
    });

    let responses = args.responses.iter().map(|response| {
        let status = &response.status;
        if let Some(ty) = &response.body {
            quote! {
                endpoint.response::<#ty>(#status);
            }
        } else {
            quote! {
                endpoint.empty_response(#status);
            }
        }
    });

    quote! {
        #[utoipa::path(#original_args)]
        #input

        fn #render_name(collector: &mut ::utoipa_ts::TypeCollector) -> ::utoipa_ts::EndpointSpec {
            let mut endpoint = ::utoipa_ts::EndpointRender::new(
                collector,
                stringify!(#fn_name),
                #method,
                #path,
            );

            #(#params)*
            #(#param_sets)*
            #(#request_body)*
            #(#responses)*

            endpoint.finish()
        }

        ::utoipa_ts::__private::inventory::submit! {
            ::utoipa_ts::Endpoint {
                name: stringify!(#fn_name),
                method: #method,
                path: #path,
                render: #render_name,
            }
        }
    }
}

struct PathArgs {
    method: Option<String>,
    path: Option<String>,
    params: Vec<Param>,
    param_sets: Vec<Type>,
    request_body: Option<Type>,
    responses: Vec<Response>,
}

impl Parse for PathArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut args = Self {
            method: None,
            path: None,
            params: Vec::new(),
            param_sets: Vec::new(),
            request_body: None,
            responses: Vec::new(),
        };

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            let key = ident.to_string();

            if input.peek(Token![=]) {
                input.parse::<Token![=]>()?;

                match key.as_str() {
                    "path" => {
                        let value: LitStr = input.parse()?;
                        args.path = Some(value.value());
                    }
                    "request_body" => {
                        args.request_body = Some(input.parse()?);
                    }
                    _ => {
                        let _: Expr = input.parse()?;
                    }
                }
            } else if input.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);

                match key.as_str() {
                    "params" => {
                        let params = parse_params(&content.parse()?)?;
                        args.params.extend(params.params);
                        args.param_sets.extend(params.param_sets);
                    }
                    "request_body" => args.request_body = parse_request_body(&content.parse()?)?,
                    "responses" => args.responses.extend(parse_responses(&content.parse()?)?),
                    _ => {}
                }
            } else if args.method.is_none() {
                args.method = Some(key.to_ascii_uppercase());
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(args)
    }
}

struct Param {
    name: String,
    ty: Type,
}

fn parse_params(tokens: &TokenStream2) -> Result<ParamList> {
    syn::parse2::<ParamList>(tokens.clone())
}

struct ParamList {
    params: Vec<Param>,
    param_sets: Vec<Type>,
}

impl Parse for ParamList {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut params = Vec::new();
        let mut param_sets = Vec::new();

        while !input.is_empty() {
            if input.peek(syn::token::Paren) {
                let content;
                parenthesized!(content in input);

                let name: LitStr = content.parse()?;
                content.parse::<Token![=]>()?;
                let ty: Type = content.parse()?;
                params.push(Param {
                    name: name.value(),
                    ty,
                });

                while !content.is_empty() {
                    let _: proc_macro2::TokenTree = content.parse()?;
                }
            } else {
                param_sets.push(input.parse()?);
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { params, param_sets })
    }
}

struct Response {
    status: String,
    body: Option<Type>,
}

fn parse_request_body(tokens: &TokenStream2) -> Result<Option<Type>> {
    syn::parse2::<RequestBody>(tokens.clone()).map(|body| body.ty)
}

struct RequestBody {
    ty: Option<Type>,
}

impl Parse for RequestBody {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut ty = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "content" {
                ty = Some(input.parse()?);
            } else {
                let _ = parse_until_comma(input)?;
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { ty })
    }
}

fn parse_responses(tokens: &TokenStream2) -> Result<Vec<Response>> {
    syn::parse2::<ResponseList>(tokens.clone()).map(|list| list.responses)
}

struct ResponseList {
    responses: Vec<Response>,
}

impl Parse for ResponseList {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut responses = Vec::new();

        while !input.is_empty() {
            let content;
            parenthesized!(content in input);
            responses.push(content.parse()?);

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { responses })
    }
}

impl Parse for Response {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut status = None;
        let mut body = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "status" => {
                    let tokens = parse_until_comma(input)?;
                    status = Some(status_tokens_to_string(tokens));
                }
                "body" => {
                    body = Some(input.parse()?);
                }
                _ => {
                    let _ = parse_until_comma(input)?;
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self {
            status: status.unwrap_or_else(|| "default".to_owned()),
            body,
        })
    }
}

fn parse_until_comma(input: ParseStream<'_>) -> Result<TokenStream2> {
    let mut tokens = TokenStream2::new();

    while !input.is_empty() && !input.peek(Token![,]) {
        let token: proc_macro2::TokenTree = input.parse()?;
        tokens.extend([token]);
    }

    Ok(tokens)
}

fn status_tokens_to_string(tokens: TokenStream2) -> String {
    let value = tokens.to_string();
    value.trim().trim_matches('"').to_owned()
}
