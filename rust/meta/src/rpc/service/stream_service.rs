use std::sync::Arc;

use risingwave_common::error::tonic_err;
use risingwave_pb::meta::stream_manager_service_server::StreamManagerService;
use risingwave_pb::meta::{
    ClusterType, CreateMaterializedViewRequest, CreateMaterializedViewResponse,
    DropMaterializedViewRequest, DropMaterializedViewResponse,
};
use tonic::{Request, Response, Status};

use crate::cluster::StoredClusterManager;
use crate::manager::{EpochGeneratorRef, IdGeneratorManagerRef, MetaSrvEnv};
use crate::stream::{StreamFragmenter, StreamManagerRef};

#[derive(Clone)]
pub struct StreamServiceImpl {
    sm: StreamManagerRef,

    id_gen_manager_ref: IdGeneratorManagerRef,
    cluster_manager: Arc<StoredClusterManager>,
    epoch_generator: EpochGeneratorRef,
}

impl StreamServiceImpl {
    pub fn new(
        sm: StreamManagerRef,
        cluster_manager: Arc<StoredClusterManager>,
        env: MetaSrvEnv,
    ) -> Self {
        StreamServiceImpl {
            sm,
            id_gen_manager_ref: env.id_gen_manager_ref(),
            cluster_manager,
            epoch_generator: env.epoch_generator_ref(),
        }
    }
}

#[async_trait::async_trait]
impl StreamManagerService for StreamServiceImpl {
    #[cfg(not(tarpaulin_include))]
    async fn create_materialized_view(
        &self,
        request: Request<CreateMaterializedViewRequest>,
    ) -> Result<Response<CreateMaterializedViewResponse>, Status> {
        let req = request.into_inner();
        let worker_count = self
            .cluster_manager
            .list_worker_node(ClusterType::ComputeNode)
            .await
            .map_err(|e| e.to_grpc_status())?
            .len();

        let mut fragmenter =
            StreamFragmenter::new(self.id_gen_manager_ref.clone(), worker_count as u32);
        let (mut graph, source_actor_ids) = fragmenter
            .generate_graph(req.get_stream_node().map_err(tonic_err)?)
            .await
            .map_err(|e| e.to_grpc_status())?;

        match self
            .sm
            .create_materialized_view(
                req.get_table_ref_id().map_err(tonic_err)?,
                &mut graph,
                source_actor_ids,
            )
            .await
        {
            Ok(()) => Ok(Response::new(CreateMaterializedViewResponse {
                status: None,
            })),
            Err(e) => Err(e.to_grpc_status()),
        }
    }

    #[cfg(not(tarpaulin_include))]
    async fn drop_materialized_view(
        &self,
        request: Request<DropMaterializedViewRequest>,
    ) -> Result<Response<DropMaterializedViewResponse>, Status> {
        let req = request.into_inner();

        match self
            .sm
            .drop_materialized_view(req.get_table_ref_id().map_err(tonic_err)?)
            .await
        {
            Ok(()) => Ok(Response::new(DropMaterializedViewResponse { status: None })),
            Err(e) => Err(e.to_grpc_status()),
        }
    }
}
