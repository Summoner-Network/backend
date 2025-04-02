#[async_trait]
pub trait CodeBase {
    async fn invoke(&self, contraction: Contraction) -> Emission;
}