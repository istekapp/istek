use tonic::{Request, Response, Status};
use tonic::transport::Server;
use tokio::sync::oneshot;
use std::pin::Pin;
use futures_util::Stream;

// Include the generated proto code
pub mod playground {
    tonic::include_proto!("playground");
    
    pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("playground_descriptor");
}

use playground::greeter_server::{Greeter, GreeterServer};
use playground::{HelloReply, HelloRequest};

/// gRPC Greeter service implementation
#[derive(Debug, Default)]
pub struct PlaygroundGreeter {}

#[tonic::async_trait]
impl Greeter for PlaygroundGreeter {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let name = request.into_inner().name;
        let reply = HelloReply {
            message: format!("Hello, {}! Welcome to Istek Playground.", name),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        Ok(Response::new(reply))
    }

    type SayHelloServerStreamStream = Pin<Box<dyn Stream<Item = Result<HelloReply, Status>> + Send>>;

    async fn say_hello_server_stream(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<Self::SayHelloServerStreamStream>, Status> {
        let name = request.into_inner().name;
        
        let stream = async_stream::stream! {
            for i in 1..=5 {
                let reply = HelloReply {
                    message: format!("Hello #{}, {}!", i, name),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                yield Ok(reply);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }
}

/// gRPC Server wrapper
pub struct GrpcServer {
    shutdown_tx: Option<oneshot::Sender<()>>,
    pub port: u16,
}

impl GrpcServer {
    pub fn new(port: u16) -> Self {
        Self {
            shutdown_tx: None,
            port,
        }
    }

    /// Start the gRPC server with reflection
    pub async fn start(&mut self) -> Result<(), String> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let addr = format!("127.0.0.1:{}", self.port)
            .parse()
            .map_err(|e| format!("Invalid address: {}", e))?;

        let greeter = PlaygroundGreeter::default();

        // Create reflection service
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(playground::FILE_DESCRIPTOR_SET)
            .build_v1()
            .map_err(|e| format!("Failed to create reflection service: {}", e))?;

        // Spawn server task
        tokio::spawn(async move {
            let result = Server::builder()
                .add_service(GreeterServer::new(greeter))
                .add_service(reflection_service)
                .serve_with_shutdown(addr, async {
                    let _ = shutdown_rx.await;
                })
                .await;

            if let Err(e) = result {
                eprintln!("gRPC server error: {}", e);
            }
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    /// Stop the gRPC server
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for GrpcServer {
    fn drop(&mut self) {
        self.stop();
    }
}
