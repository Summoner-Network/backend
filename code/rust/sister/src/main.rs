use wasmtime::component::bindgen;

bindgen!({
    world: "demo",
    path:  "../sister-contract/wit",   // <-- path to the directory that contains demo.wit
    async: true                       // generate `async fn` in the traits
});

fn main() {
    println!("Hello, world!");
}
