use brother::pb::{
    brother_server::{Brother, BrotherServer},
    GetObjectRequest, GetObjectResponse,            // …and the rest
    PutObjectRequest,  PutObjectResponse,
    RemoveObjectRequest, RemoveObjectResponse,
    CreateAssociationRequest, CreateAssociationResponse,
    RemoveAssociationRequest, RemoveAssociationResponse,
    GetAssociationsRequest,  GetAssociationsResponse,
};
use tonic::{transport::Server, Request, Response, Status};

/// Your domain logic goes into a type that implements the generated `Brother` trait.
#[derive(Default)]
struct BrotherService;

#[async_trait::async_trait]
impl Brother for BrotherService {
    async fn get_object(
        &self,
        _req: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        // <── TODO: real implementation
        Ok(Response::new(GetObjectResponse { object: None }))
    }

    async fn put_object(
        &self,
        _req: Request<PutObjectRequest>,
    ) -> Result<Response<PutObjectResponse>, Status> {
        Ok(Response::new(PutObjectResponse { success: true }))
    }

    async fn remove_object(
        &self,
        _req: Request<RemoveObjectRequest>,
    ) -> Result<Response<RemoveObjectResponse>, Status> {
        Ok(Response::new(RemoveObjectResponse { success: true }))
    }

    async fn create_association(
        &self,
        _req: Request<CreateAssociationRequest>,
    ) -> Result<Response<CreateAssociationResponse>, Status> {
        Ok(Response::new(CreateAssociationResponse { success: true }))
    }

    async fn remove_association(
        &self,
        _req: Request<RemoveAssociationRequest>,
    ) -> Result<Response<RemoveAssociationResponse>, Status> {
        Ok(Response::new(RemoveAssociationResponse { success: true }))
    }

    async fn get_associations(
        &self,
        _req: Request<GetAssociationsRequest>,
    ) -> Result<Response<GetAssociationsResponse>, Status> {
        Ok(Response::new(GetAssociationsResponse { associations: vec![] }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr          = "[::1]:42069".parse()?;
    let svc_impl      = BrotherService::default();
    let grpc_service  = BrotherServer::new(svc_impl);

    println!("Brother gRPC server listening on {addr}");
    Server::builder().add_service(grpc_service).serve(addr).await?;
    Ok(())
}
