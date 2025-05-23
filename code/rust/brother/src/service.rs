use std::sync::Arc;

use crate::db::{db_err, PgPool};               // whatever module you put the pool in
use brother::pb::{
    brother_server::Brother, Association, CreateAssociationRequest, CreateAssociationResponse, GetAssociationsRequest, GetAssociationsResponse, GetObjectRequest, GetObjectResponse, Object, PutObjectRequest, PutObjectResponse, RemoveAssociationRequest, RemoveAssociationResponse, RemoveObjectRequest, RemoveObjectResponse
};
use tonic::{Request, Response, Status};
use tracing::instrument;
use sqlx::Row;

// ──────────────────────────────────────────────────────────────
//  The service implementation
// ──────────────────────────────────────────────────────────────
#[derive(Clone)]
pub struct BrotherService {
    db: Arc<PgPool>,
}

impl BrotherService {
    pub fn new(db: PgPool) -> Self {
        Self { db: Arc::new(db) }
    }

    fn attrs_to_json(
        map: &std::collections::HashMap<String, String>,
    ) -> serde_json::Value {
        serde_json::to_value(map).unwrap()
    }

    fn json_to_attrs(
        json: serde_json::Value,
    ) -> std::collections::HashMap<String, String> {
        serde_json::from_value(json).unwrap_or_default()
    }
}

#[tonic::async_trait]
impl Brother for BrotherService {
    // ─────────────────── Objects ───────────────────
    #[instrument(skip(self))]
    async fn get_object(
        &self,
        req: Request<GetObjectRequest>,
    ) -> Result<Response<GetObjectResponse>, Status> {
        let GetObjectRequest { otype, id } = req.into_inner();

        let row = sqlx::query(
            r#"SELECT version, attributes
                 FROM tao.objects
                WHERE tenant = $1 AND type = $2 AND id = $3"#,
        )
        .bind(0_i64)             // TODO: resolve tenant from auth
        .bind(otype as i32)
        .bind(id as i64)
        .fetch_optional(&*self.db)
        .await
        .map_err(db_err)?;

        let object = row.map(|r| Object {
            tenant: 0,
            r#type: otype,
            id,
            version: r.get::<i32, _>("version") as u32,
            attributes: Self::json_to_attrs(r.get("attributes")),
        });

        Ok(Response::new(GetObjectResponse { object }))
    }

    #[instrument(skip(self))]
    async fn put_object(
        &self,
        req: Request<PutObjectRequest>,
    ) -> Result<Response<PutObjectResponse>, Status> {
        let Some(obj) = req.into_inner().object else {
            return Err(Status::invalid_argument("object is required"));
        };

        let success: bool = sqlx::query_scalar(
            r#"SELECT tao.tao_upsert_object($1,$2,$3,$4,$5)"#,
        )
        .bind(obj.tenant as i64)
        .bind(obj.r#type as i32)
        .bind(obj.id as i64)
        .bind(obj.version as i32)
        .bind(Self::attrs_to_json(&obj.attributes))
        .fetch_one(&*self.db)
        .await
        .map_err(db_err)?;

        Ok(Response::new(PutObjectResponse {
            success,
            id: obj.id,
        }))
    }

    #[instrument(skip(self))]
    async fn remove_object(
        &self,
        req: Request<RemoveObjectRequest>,
    ) -> Result<Response<RemoveObjectResponse>, Status> {
        let RemoveObjectRequest { otype, id } = req.into_inner();

        let success: bool = sqlx::query_scalar(
            r#"SELECT tao.tao_delete_object($1,$2,$3)"#,
        )
        .bind(0_i64)
        .bind(otype as i32)
        .bind(id as i64)
        .fetch_one(&*self.db)
        .await
        .map_err(db_err)?;

        Ok(Response::new(RemoveObjectResponse { success }))
    }

    // ─────────────────── Associations ───────────────────
    #[instrument(skip(self))]
    async fn create_association(
        &self,
        req: Request<CreateAssociationRequest>,
    ) -> Result<Response<CreateAssociationResponse>, Status> {
        let Some(a) = req.into_inner().association else {
            return Err(Status::invalid_argument("association is required"));
        };

        sqlx::query(
            r#"SELECT tao.tao_upsert_association($1,$2,$3,$4,$5,$6,$7)"#,
        )
        .bind(a.tenant as i64)
        .bind(&a.r#type)
        .bind(a.source_id as i64)
        .bind(a.target_id as i64)
        .bind(a.time as i64)
        .bind(a.position as i64)
        .bind(Self::attrs_to_json(&a.attributes))
        .execute(&*self.db)
        .await
        .map_err(db_err)?;

        Ok(Response::new(CreateAssociationResponse { success: true }))
    }

    #[instrument(skip(self))]
    async fn remove_association(
        &self,
        req: Request<RemoveAssociationRequest>,
    ) -> Result<Response<RemoveAssociationResponse>, Status> {
        let RemoveAssociationRequest {
            atype,
            src_id,
            tar_tid,
            ..
        } = req.into_inner();

        let success: bool = sqlx::query_scalar(
            r#"SELECT tao.tao_delete_association($1,$2,$3,$4)"#,
        )
        .bind(0_i64)
        .bind(&atype)
        .bind(src_id as i64)
        .bind(tar_tid as i64)
        .fetch_one(&*self.db)
        .await
        .map_err(db_err)?;

        Ok(Response::new(RemoveAssociationResponse { success }))
    }

    #[instrument(skip(self))]
    async fn get_associations(
        &self,
        req: Request<GetAssociationsRequest>,
    ) -> Result<Response<GetAssociationsResponse>, Status> {
        let GetAssociationsRequest {
            src_id,
            atype,
            tar_id,
            above,
            limit,
            ..
        } = req.into_inner();

        let rows = sqlx::query(
            r#"
            SELECT target_id, time, position, attributes
              FROM tao.associations
             WHERE tenant    = $1
               AND type      = $2
               AND source_id = $3
               AND target_id = $4
               AND position  > $5
             ORDER BY position DESC
             LIMIT $6
            "#,
        )
        .bind(0_i64)
        .bind(&atype)
        .bind(src_id as i64)
        .bind(tar_id as i64)
        .bind(above as i64)
        .bind(limit as i32)
        .fetch_all(&*self.db)
        .await
        .map_err(db_err)?;

        let associations = rows
            .into_iter()
            .map(|r| Association {
                tenant: 0,
                r#type: atype.clone(),
                source_id: src_id,
                target_id: r.get::<i64, _>("target_id") as u64,
                time: r.get::<i64, _>("time") as u64,
                position: r.get::<i64, _>("position") as u64,
                attributes: Self::json_to_attrs(r.get("attributes")),
            })
            .collect();

        Ok(Response::new(GetAssociationsResponse { associations }))
    }
}


