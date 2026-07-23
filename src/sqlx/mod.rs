
#[async_trait::async_trait]
pub trait SqlxSettings{
    async fn get_connection_string(&self) -> String;
}