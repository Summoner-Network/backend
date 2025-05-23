#![allow(warnings)]          // you can delete this when you’re happy

// bring the generated code into scope
mod bindings;

// exported interface from the `demo` world
use bindings::exports::brother::guest::contract;

// any shared types you need
use bindings::brother::guest::common::{Header, Headers};

struct Component;

// implement the generated Guest trait
impl contract::Guest for Component {

    fn deploy(payload: Headers) -> () {
        // Initialize the contract; this is called once on deployment
        todo!()
    }

    fn render(routing: String) -> String {
        // Implement marketing page, dashboards and usable interfaces
        todo!()
    }

    fn invoke(method: String, payload: Headers) -> Result<Headers, u64> {
        // TODO: real logic – this just echoes the request
        let mut out = payload;
        out.push(Header { key: "handled-by".into(), value: method });
        Ok(out)
    }
    
}

// tell cargo-component what to export
bindings::export!(Component with_types_in bindings);
