//! The HTTP listeners (the web app and API, the Plex-compatible API): like
//! `axum::serve`, but a client gets [`HEADER_TIMEOUT`] to send a request's
//! headers, so connections opened and never used (or fed a byte at a time)
//! don't pile up until pley runs out of them.

use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::extract::Request;
use axum::response::Response;
use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower::{Service, ServiceExt};

/// How long a client has to send the headers of a request (the first one,
/// or the next one on a connection kept open).
pub const HEADER_TIMEOUT: Duration = Duration::from_secs(30);

/// Serves `service` on `listener` until `shutdown` completes; then stops
/// taking connections, lets the requests in progress finish (streams end
/// with the server's own stop signal) and returns once every connection is
/// closed. Handlers see the client's address as `ConnectInfo<SocketAddr>`.
pub async fn serve<S>(listener: TcpListener, service: S, shutdown: impl Future<Output = ()> + Send + 'static)
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    serve_with(listener, service, shutdown, HEADER_TIMEOUT).await;
}

async fn serve_with<S>(
    listener: TcpListener,
    service: S,
    shutdown: impl Future<Output = ()> + Send + 'static,
    header_timeout: Duration,
) where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    let stop = CancellationToken::new();
    let connections = TaskTracker::new();
    tokio::pin!(shutdown);
    loop {
        let (stream, peer) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(connection) => connection,
                Err(err) => {
                    if !is_connection_error(&err) {
                        // Out of file descriptors, most likely: wait for some to close.
                        tracing::warn!("cannot accept a connection: {err}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    continue;
                }
            },
            () = &mut shutdown => break,
        };
        let _ = stream.set_nodelay(true);
        let service = service.clone();
        let stop = stop.clone();
        connections.spawn(async move {
            let service = hyper::service::service_fn(move |mut request: hyper::Request<Incoming>| {
                request.extensions_mut().insert(ConnectInfo(peer));
                service.clone().oneshot(request.map(Body::new))
            });
            let connection = hyper::server::conn::http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(header_timeout)
                .serve_connection(TokioIo::new(stream), service)
                .with_upgrades();
            tokio::pin!(connection);
            tokio::select! {
                result = connection.as_mut() => log_end(result, peer),
                () = stop.cancelled() => {
                    connection.as_mut().graceful_shutdown();
                    log_end(connection.await, peer);
                }
            }
        });
    }
    drop(listener);
    stop.cancel();
    connections.close();
    connections.wait().await;
}

fn log_end(result: Result<(), hyper::Error>, peer: SocketAddr) {
    if let Err(err) = result {
        tracing::trace!("connection from {peer} ended: {err}");
    }
}

/// Errors of one connection (it went away before it was accepted), not of
/// the listener.
fn is_connection_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::ConnectionRefused | io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const TIMEOUT: Duration = Duration::from_millis(300);

    async fn server() -> (SocketAddr, CancellationToken, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/who",
            axum::routing::get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move { peer.ip().to_string() }),
        );
        let stop = CancellationToken::new();
        let task = tokio::spawn(serve_with(listener, app, stop.clone().cancelled_owned(), TIMEOUT));
        (addr, stop, task)
    }

    #[tokio::test]
    async fn answers_with_the_clients_address_and_stops() {
        let (addr, stop, task) = server().await;
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream.write_all(b"GET /who HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n").await.unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).await.unwrap();
        assert!(answer.starts_with("HTTP/1.1 200") && answer.ends_with("127.0.0.1"), "{answer}");
        stop.cancel();
        tokio::time::timeout(Duration::from_secs(5), task).await.expect("stopped").unwrap();
    }

    #[tokio::test]
    async fn slow_clients_are_let_go() {
        let (addr, _stop, _task) = server().await;
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        stream.write_all(b"GET /who HTTP/1.1\r\n").await.unwrap();
        // Never finishing the headers: the connection is closed after the timeout.
        let mut answer = Vec::new();
        let read = tokio::time::timeout(TIMEOUT * 10, stream.read_to_end(&mut answer)).await;
        assert!(read.is_ok(), "still open after {:?}", TIMEOUT * 10);
        assert!(!String::from_utf8_lossy(&answer).contains("200 OK"));
    }
}
