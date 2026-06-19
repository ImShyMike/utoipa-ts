use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
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
/// - `params(...)`, including parameter location and description metadata
/// - `request_body = Type`
/// - `request_body(content = Type, ...)`
/// - `responses((status = 200, body = Type), ...)`, including descriptions, content types, and headers
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
        let location = param.location.to_tokens();
        let description = option_string_tokens(param.description.as_deref());
        quote! {
            endpoint.param_with_location::<#ty>(#name, #location, #description);
        }
    });

    let param_sets = args.param_sets.iter().map(|ty| {
        quote! {
            endpoint.params::<#ty>();
        }
    });

    let request_body = args.request_body.iter().map(|ty| {
        let body = &ty.ty;
        let description = option_string_tokens(ty.description.as_deref());
        quote! {
            endpoint.request_body_with_description::<#body>(#description);
        }
    });

    let responses = args.responses.iter().map(|response| {
        let status = &response.status;
        let description = option_string_tokens(response.description.as_deref());
        let content_type = option_string_tokens(response.content_type.as_deref());
        let headers = response.headers.iter().map(|header| {
            let name = &header.name;
            let ty = &header.ty;
            let description = option_string_tokens(header.description.as_deref());
            quote! {
                endpoint.response_header::<#ty>(#status, #name, #description);
            }
        });

        if let Some(ty) = &response.body {
            quote! {
                endpoint.response_with_metadata::<#ty>(#status, #description, #content_type);
                #(#headers)*
            }
        } else {
            quote! {
                endpoint.empty_response_with_metadata(#status, #description, #content_type);
                #(#headers)*
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
    request_body: Option<RequestBody>,
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
                        args.request_body = Some(RequestBody {
                            ty: input.parse()?,
                            description: None,
                        });
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
    location: ParamLocation,
    description: Option<String>,
}

enum ParamLocation {
    Query,
    Header,
    Path,
    Cookie,
}

impl ParamLocation {
    fn to_tokens(&self) -> TokenStream2 {
        match self {
            Self::Query => quote!(::utoipa_ts::ParameterLocation::Query),
            Self::Header => quote!(::utoipa_ts::ParameterLocation::Header),
            Self::Path => quote!(::utoipa_ts::ParameterLocation::Path),
            Self::Cookie => quote!(::utoipa_ts::ParameterLocation::Cookie),
        }
    }
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
                let mut location = ParamLocation::Path;
                let mut description = None;

                while !content.is_empty() {
                    if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                        continue;
                    }

                    if content.peek(Ident) {
                        let key: Ident = content.parse()?;
                        match key.to_string().as_str() {
                            "Query" => location = ParamLocation::Query,
                            "Header" => location = ParamLocation::Header,
                            "Path" => location = ParamLocation::Path,
                            "Cookie" => location = ParamLocation::Cookie,
                            "description" if content.peek(Token![=]) => {
                                content.parse::<Token![=]>()?;
                                description = Some(parse_string_value(&content)?);
                            }
                            _ if content.peek(Token![=]) => {
                                content.parse::<Token![=]>()?;
                                let _ = parse_until_comma(&content)?;
                            }
                            _ => {}
                        }
                    } else {
                        let _: proc_macro2::TokenTree = content.parse()?;
                    }
                }

                params.push(Param {
                    name: name.value(),
                    ty,
                    location,
                    description,
                });
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
    content_type: Option<String>,
    headers: Vec<ResponseHeader>,
    description: Option<String>,
}

struct ResponseHeader {
    name: String,
    ty: Type,
    description: Option<String>,
}

fn parse_request_body(tokens: &TokenStream2) -> Result<Option<RequestBody>> {
    syn::parse2::<RequestBodyParser>(tokens.clone()).map(|body| {
        body.ty.map(|ty| RequestBody {
            ty,
            description: body.description,
        })
    })
}

struct RequestBody {
    ty: Type,
    description: Option<String>,
}

struct RequestBodyParser {
    ty: Option<Type>,
    description: Option<String>,
}

impl Parse for RequestBodyParser {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut ty = None;
        let mut description = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            if key == "content" {
                ty = Some(input.parse()?);
            } else if key == "description" {
                description = Some(parse_string_value(input)?);
            } else {
                let _ = parse_until_comma(input)?;
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { ty, description })
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
        let mut content_type = None;
        let mut headers = Vec::new();
        let mut description = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            match key.to_string().as_str() {
                "status" => {
                    input.parse::<Token![=]>()?;
                    let tokens = parse_until_comma(input)?;
                    status = Some(status_tokens_to_string(tokens));
                }
                "body" => {
                    input.parse::<Token![=]>()?;
                    body = Some(input.parse()?);
                }
                "content_type" => {
                    input.parse::<Token![=]>()?;
                    content_type = parse_optional_string_literal(input)?;
                }
                "headers" if input.peek(syn::token::Paren) => {
                    let content;
                    parenthesized!(content in input);
                    headers.extend(syn::parse2::<ResponseHeaderList>(content.parse()?)?.headers);
                }
                "description" => {
                    input.parse::<Token![=]>()?;
                    description = Some(parse_string_value(input)?);
                }
                _ => {
                    if input.peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                    }
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
            content_type,
            headers,
            description,
        })
    }
}

struct ResponseHeaderList {
    headers: Vec<ResponseHeader>,
}

impl Parse for ResponseHeaderList {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut headers = Vec::new();

        while !input.is_empty() {
            let content;
            parenthesized!(content in input);
            headers.push(content.parse()?);

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Self { headers })
    }
}

impl Parse for ResponseHeader {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name: LitStr = input.parse()?;
        let mut ty = syn::parse_str::<Type>("String")?;
        let mut description = None;

        if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            ty = input.parse()?;
        }

        while !input.is_empty() {
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                continue;
            }

            if input.peek(Ident) {
                let key: Ident = input.parse()?;
                match key.to_string().as_str() {
                    "description" if input.peek(Token![=]) => {
                        input.parse::<Token![=]>()?;
                        description = Some(parse_string_value(input)?);
                    }
                    _ if input.peek(Token![=]) => {
                        input.parse::<Token![=]>()?;
                        let _ = parse_until_comma(input)?;
                    }
                    _ => {}
                }
            } else {
                let _: proc_macro2::TokenTree = input.parse()?;
            }
        }

        Ok(Self {
            name: name.value(),
            ty,
            description,
        })
    }
}

fn parse_optional_string_literal(input: ParseStream<'_>) -> Result<Option<String>> {
    if input.peek(LitStr) {
        let value: LitStr = input.parse()?;
        Ok(Some(value.value()))
    } else {
        let _ = parse_until_comma(input)?;
        Ok(None)
    }
}

fn parse_string_value(input: ParseStream<'_>) -> Result<String> {
    if input.peek(LitStr) {
        let value: LitStr = input.parse()?;
        Ok(value.value())
    } else {
        Ok(parse_until_comma(input)?.to_string())
    }
}

fn option_string_tokens(value: Option<&str>) -> TokenStream2 {
    value.map_or_else(
        || quote!(None),
        |value| {
            let value = LitStr::new(value, Span::call_site());
            quote!(Some(#value))
        },
    )
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
