# poem-route-macro

Provides a simple macro to ease the definition of routes in [Poem].

## Example

Here is an example use of the macro to construct a Poem [`Route`] that has a number of end-points
and a nested static files endpoint.

```rust
use poem::{endpoint::StaticFilesEndpoint, EndpointExt, IntoEndpoint, Route};

fn build_routes(my_data: MyData) -> impl IntoEndpoint {
    define_routes!(Route::new(), {
        // Nest a static files endpoint
        *"/static"      { StaticFilesEndpoint::new("./static") }

        // Standard routes
        "/"             index::index        GET
        "/pastes"       paste::pastes       GET
        "/pastes/:id"   paste::paste        GET POST

        // A nested route for administration
        *"/admin"       { admin::build_routes() }
    })
    .data(my_data)
}
```

The above will generate the following code:

```rust
fn build_routes() -> Route {
    Route::new()
        .nest("/static", StaticFilesEndpoint::new("./static"))
        .at("/", poem::get(index::get_index))
        .at("/pastes", poem::get(paste::get_pastes))
        .at("/pastes/:id", poem::get(paste::get_paste).post(post_paste))
        .nest("/admin", admin::build_routes())
}
```

## How to use

The optional first argument to the `define_routes` macro is the expression to which all the routes
are applied in a builder pattern. If this expression is missing, it defaults to `Route::new()`. As
such, these two are equivalent:

```rust
fn build_routes_no_expr() -> Route {
    define_routes!({
        "/" index GET
    })
}

fn build_routes_with_expr() -> Route {
    define_routes!(Route::new(), {
        "/" index GET
    })
}
```

This can be useful, as you might want to have something _before_ the endpoints are added
builder-style to the router.

After the optional expression, the routes are specified, wrapped in braces.

```rust
define_routes!({
    // routes go here
})
```

There are two routes supported: nested and normal.

### Nested Endpoints

A nested route is indicated by an asterisk, followed by the path on which to nest the endpoint.

```rust
define_routes!({
    // A nested route under "/static", e.g. "/static/foo"
    *"/static" { my_nested_router }
})
```

After the path string there should be an expression block, being some Rust statements in braces.
This code should evaluate to something that implements `IntoEndpoint`.

This can be useful to nest multiple routers together:

```rust
fn admin_routes() -> Route {
    define_route!({
        "/"         admin::index    GET
        "/users"    admin::users    GET POST
    })
}

fn post_routes() -> Route {
    define_route!({
        "/"         posts::posts    GET
        "/:id"      posts::post     GET POST
    })
}

fn build_routes() -> Route {
    define_route!({
        *"/admin"   { admin_routes() }
        *"/posts"   { post_routes() }
    })
}
```

### Normal Endpoints

Normal endpoints are specified by the path string, then the handler name template, followed by a
list of methods. The handler name template is a Rust path, such as `handler` or `module::handler`.
This path is more like that found in a `use` statement, in that it cannot include generic parameters
in any of its segments.

The handler name template is modified for each method by prefixing the method, lower-case, to the
last element in the handler name template, separated by an underscore. For example, if the handler
name template was `s3::bucket` and the methods where `GET` and `POST`, the handler names generated
will be `s3::get_bucket` and `s3::post_bucket`

Consider the following set of handlers:

```rust
// Let's define a handler in our current module that we'll use for the "/" route.
#[handler]
fn get_index() -> poem::Result<()> { todo!() }

// Define a couple more handlers for signing in under "/user/signin"
#[handler]
fn get_user_signin() -> poem::Result<()> { todo!() }
#[handler]
fn post_user_signin() -> poem::Result<()> { todo!() }

// Now let's define a couple of handlers for S3 buckets in their own module.
mod s3 {
    #[handler]
    pub fn get_bucket() -> poem::Result<()> { todo!() }

    #[handler]
    pub fn post_bucket() -> poem::Result<()> { todo!() }
}
```

We can then wire-up these handlers with the `define_route` as follows:

```rust
fn build_routes() -> Route {
    define_route!({
        // Handler template 'index' becomes 'get_index'
        "/"             index           GET

        // Handler template 'user_signin' becomes 'get_user_signin' and 'post_user_signin'
        "/user/signin"  user_signin     GET POST

        // Wire up the S3 bucket handlers the same way.
        "/s3/:bucket"   s3::bucket      GET POST
    })
}
```

## Grammar

The grammar for this simple routing table DSL is given in the following rough eBNF:

```ebnf
body = { EXPR "," } "{" routes "}" ;

routes = route { route } ;

route = "*" LIT_STR EXPR_BLOCK
      |     LIT_STR path methods
      ;

path = IDENT { "::" IDENT } ;

methods = method { method } ;

method = "GET" | "POST" | "DELETE" | "PUT" ;
```
