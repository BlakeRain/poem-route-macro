use proc_macro::TokenStream;
use quote::{format_ident, quote, IdentFragment, ToTokens, TokenStreamExt};
use syn::{
    braced,
    parse::{Parse, ParseStream},
    parse_macro_input, Token,
};

struct NestedRoute {
    path: syn::LitStr,
    endpoint: syn::ExprBlock,
}

impl Parse for NestedRoute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<Token![*]>()?;
        let path = input.parse()?;
        let endpoint = input.parse()?;
        Ok(Self { path, endpoint })
    }
}

impl NestedRoute {
    fn cleanup_endpoint(&self) -> proc_macro2::TokenStream {
        let Self { endpoint, .. } = self;

        // This is a cheeky shortcut to avoid warnings from Clippy insisting that we remove the
        // braces around a method argument. This is because the nested endpoint might be a simple
        // expression that Clippy, quite rightly, asserts need not be wrapped in braces. To reduce
        // the warnings from the generated code, if we find a single expression in the ExprBlock,
        // then we just reduce to that expression.
        if endpoint.block.stmts.len() == 1 {
            if let Some(syn::Stmt::Expr(expr, _)) = endpoint.block.stmts.first() {
                return quote! {
                    #expr
                };
            }
        }

        quote! {
            #endpoint
        }
    }

    fn render(&self) -> proc_macro2::TokenStream {
        let Self { path, .. } = self;
        let endpoint = self.cleanup_endpoint();

        quote! {
          .nest(#path, #endpoint)
        }
    }
}

mod keyword {
    syn::custom_keyword!(GET);
    syn::custom_keyword!(POST);
    syn::custom_keyword!(PUT);
    syn::custom_keyword!(DELETE);
}

enum Method {
    Get(keyword::GET),
    Post(keyword::POST),
    Put(keyword::PUT),
    Delete(keyword::DELETE),
}

impl Method {
    fn render(&self) -> &'static str {
        match self {
            Self::Get(_) => "get",
            Self::Post(_) => "post",
            Self::Put(_) => "put",
            Self::Delete(_) => "delete",
        }
    }
}

impl IdentFragment for Method {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self.render(), f)
    }

    fn span(&self) -> Option<proc_macro2::Span> {
        Some(match self {
            Self::Get(kw) => kw.span,
            Self::Post(kw) => kw.span,
            Self::Put(kw) => kw.span,
            Self::Delete(kw) => kw.span,
        })
    }
}

impl ToTokens for Method {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = match self {
            Self::Get(kw) => proc_macro2::Ident::new("get", kw.span),
            Self::Post(kw) => proc_macro2::Ident::new("post", kw.span),
            Self::Put(kw) => proc_macro2::Ident::new("put", kw.span),
            Self::Delete(kw) => proc_macro2::Ident::new("delete", kw.span),
        };

        tokens.append(ident);
    }
}

impl Parse for Method {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(keyword::GET) {
            Ok(Self::Get(input.parse::<keyword::GET>()?))
        } else if lookahead.peek(keyword::POST) {
            Ok(Self::Post(input.parse::<keyword::POST>()?))
        } else if lookahead.peek(keyword::PUT) {
            Ok(Self::Put(input.parse::<keyword::PUT>()?))
        } else if lookahead.peek(keyword::DELETE) {
            Ok(Self::Delete(input.parse::<keyword::DELETE>()?))
        } else {
            Err(lookahead.error())
        }
    }
}

struct StandardRoute {
    path: syn::LitStr,
    ident: syn::Path,
    methods: Vec<Method>,
}

impl Parse for StandardRoute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;
        let ident = input.parse()?;
        let methods = {
            let mut methods = Vec::new();

            while !input.is_empty() {
                let lookahead = input.lookahead1();
                if !lookahead.peek(syn::Ident) {
                    break;
                }

                methods.push(input.parse()?);
            }

            methods
        };

        Ok(Self {
            path,
            ident,
            methods,
        })
    }
}

fn apply_method(ident: &syn::Ident, method: &Method) -> syn::Ident {
    format_ident!("{}_{}", method, ident)
}

fn apply_method_path(head: bool, path: &syn::Path, method: &Method) -> proc_macro2::TokenStream {
    let mut path = path.clone();
    if let Some(last) = path.segments.last_mut() {
        last.ident = apply_method(&last.ident, method)
    }

    if head {
        quote! {
          poem::#method(#path)
        }
    } else {
        quote! {
          . #method(#path)
        }
    }
}

impl StandardRoute {
    fn render(&self) -> proc_macro2::TokenStream {
        let Self {
            path,
            ident,
            methods,
        } = self;

        let mut builder = Vec::new();
        for method in methods {
            builder.push(apply_method_path(builder.is_empty(), ident, method));
        }

        quote! {
          .at(#path, #(#builder)*)
        }
    }
}

enum Route {
    Nested(NestedRoute),
    Standard(StandardRoute),
}

impl Parse for Route {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![*]) {
            Ok(Self::Nested(input.parse()?))
        } else {
            Ok(Self::Standard(input.parse()?))
        }
    }
}

impl Route {
    fn render(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Nested(nested) => nested.render(),
            Self::Standard(standard) => standard.render(),
        }
    }
}

struct Routes {
    route: proc_macro2::TokenStream,
    routes: Vec<Route>,
}

impl Parse for Routes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let route = {
            let lookahead = input.lookahead1();
            if lookahead.peek(syn::token::Brace) {
                quote! {
                    poem::Route::new()
                }
            } else {
                let route = input.parse::<syn::Expr>()?;
                input.parse::<Token![,]>()?;

                quote! {
                  #route
                }
            }
        };

        let content;
        braced!(content in input);
        let mut routes = Vec::new();

        while !content.is_empty() {
            routes.push(content.parse()?);
        }

        Ok(Self { route, routes })
    }
}

impl Routes {
    fn render(&self) -> proc_macro2::TokenStream {
        let Self { route, routes } = self;
        let routes = routes.iter().map(Route::render);

        quote! {
          #route #(#routes)*
        }
    }
}

/// Helpful macro to simplify definition of routes.
///
/// The first argument to the macro is the name of the `Route` object to which endpoints are to be
/// added. Each endpoint is defined as a literal string, followed by an identifier, followed by a
/// list of one or more methods.
///
/// For each method, an identifier is formed from the lower-case method, an underscore, and then
/// the route identifier. For example, a route identifier of `foo` with methods `GET` and `POST`
/// will generate identifiers `get_foo` and `post_foo`.
///
/// The name of a route can be a qualified identifier, such as "module::foo". Any method-specific
/// modifications are applied to the last identifier in the path: "module::get_foo".
///
/// Routes can also be nested by prefixing the route string with an asterisk. In this case, a block
/// expression is expected after the path string.
///
/// As an example, consider the following:
///
/// ```ignore
/// fn add_routes(route: Route) -> Route {
///     define_routes!(route {
///       *"/static" { StaticFilesEndpoint::new("./static") }
///
///        "/"       index         GET
///        "/foo"    module::foo   GET POST
///        "/bar"    bar           GET POST PUT
///     });
///
///     route
/// }
/// ```
///
/// This will generate the Rust code equivalent to:
///
/// ```ignore
/// fn add_routes(route: Route) -> Route {
///     route.nest("/static", StaticFilesEndpoint::new("./static"))
///          .at("/", get(get_root))
///          .at("/foo", get(module::get_foo).post(module::post_foo))
///          .at("/bar", get(get_bar).post(post_bar).put(put_bar))
/// }
/// ```
///
/// Note that the nested static files endpoint is wrapped in braces. This is to help keep the
/// grammar simple. If the braces are not really needed, they will be stripped from the generated
/// code.
///
/// The grammar for the route specification is as follows:
///
/// ```plain
/// routes := route { route }
///
/// route := nested-route | plain-route
///
/// nested-route := "*" LIT_STR EXPR_BLOCK
///
/// plain-route := LIT_STR path methods
///
/// path := IDENT { "::" IDENT }
///
/// methods := method { method }
///
/// method := "GET" | "POST" | "PUT" | "DELETE"
/// ```
///
#[proc_macro]
pub fn define_routes(input: TokenStream) -> TokenStream {
    (parse_macro_input!(input as Routes)).render().into()
}
