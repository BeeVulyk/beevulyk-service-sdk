use std::pin::Pin;
use std::task::{Context, Poll};

use beevulyk_grpc_extensions::hyper;
use beevulyk_grpc_extensions::tonic::body::Body;
use tower::{Layer, Service};

#[derive(Debug, Clone, Default)]
pub struct GrpcTracingMiddlewareLayer;

impl<S> Layer<S> for GrpcTracingMiddlewareLayer {
    type Service = GrpcTracingMiddleware<S>;

    fn layer(&self, service: S) -> Self::Service {
        GrpcTracingMiddleware { inner: service }
    }
}

#[derive(Debug, Clone)]
pub struct GrpcTracingMiddleware<S> {
    inner: S,
}

type BoxFuture<'a, T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

impl<S> Service<hyper::Request<Body>> for GrpcTracingMiddleware<S>
where
    S: Service<hyper::Request<Body>, Response = hyper::Response<Body>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: hyper::Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        Box::pin(async move {
            let response = inner.call(req).await?;
            Ok(response)
        })
    }
}
