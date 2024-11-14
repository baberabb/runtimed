//! Interfacing and connecting with Jupyter kernels
//!
//! This module provides structures for understanding the connection information,
//! existing jupyter runtimes, and a client with ZeroMQ sockets to
//! communicate with the kernels.

#[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
use crate::messaging::{
    ClientControlConnection, ClientHeartbeatConnection, ClientIoPubConnection,
    ClientShellConnection, ClientStdinConnection, Connection, KernelControlConnection,
    KernelHeartbeatConnection, KernelIoPubConnection, KernelShellConnection, KernelStdinConnection,
};

#[cfg(feature = "tokio-runtime")]
use tokio::net::TcpListener;

#[cfg(feature = "async-dispatcher-runtime")]
use async_std::net::TcpListener;

#[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
use zeromq::Socket as _;

use serde::{Deserialize, Serialize};

use rand::{distributions::Alphanumeric, Rng};

use anyhow::Result;
use std::net::{IpAddr, SocketAddr};

/// Connection information for a Jupyter kernel, as represented in a
/// JSON connection file.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionInfo {
    pub ip: String,
    pub transport: String,
    pub shell_port: u16,
    pub iopub_port: u16,
    pub stdin_port: u16,
    pub control_port: u16,
    pub hb_port: u16,
    pub key: String,
    pub signature_scheme: String,
    // Ignore if not present
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_name: Option<String>,
}

/// Generate a random key in the style of Jupyter: "AAAAAAAA-AAAAAAAAAAAAAAAAAAAAAAAA"
/// (A comment in the Python source indicates the author intended a dash
/// every 8 characters, but only actually does it for the first chunk)
pub fn jupyter_style_key() -> String {
    let a: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    let b: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(24)
        .map(char::from)
        .collect();
    format!("{}-{}", a, b,)
}

/// Private function for finding a set of open ports. This function creates a listener with the port set to 0.
/// The listener is closed at the end of the function when the listener goes out of scope.
///
/// This of course opens a race condition in between closing the port and usage by a kernel,
/// but it is inherent to the design of the Jupyter protocol.
pub async fn peek_ports(ip: IpAddr, num: usize) -> Result<Vec<u16>> {
    let mut addr_zeroport: SocketAddr = SocketAddr::new(ip, 0);
    addr_zeroport.set_port(0);

    let mut ports: Vec<u16> = Vec::new();
    for _ in 0..num {
        let listener = TcpListener::bind(addr_zeroport).await?;
        let bound_port = listener.local_addr()?.port();
        ports.push(bound_port);
    }
    Ok(ports)
}

impl ConnectionInfo {
    /// format the iopub url for a ZeroMQ connection
    pub fn iopub_url(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.iopub_port)
    }

    /// format the shell url for a ZeroMQ connection
    pub fn shell_url(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.shell_port)
    }

    /// format the stdin url for a ZeroMQ connection
    pub fn stdin_url(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.stdin_port)
    }

    /// format the control url for a ZeroMQ connection
    pub fn control_url(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.control_port)
    }

    /// format the heartbeat url for a ZeroMQ connection
    pub fn hb_url(&self) -> String {
        format!("{}://{}:{}", self.transport, self.ip, self.hb_port)
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_kernel_iopub_connection(
        &self,
        session_id: &str,
    ) -> anyhow::Result<KernelIoPubConnection> {
        let endpoint = self.iopub_url();

        let mut socket = zeromq::PubSocket::new();
        socket.bind(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_kernel_shell_connection(
        &self,
        session_id: &str,
    ) -> anyhow::Result<KernelShellConnection> {
        let endpoint = self.shell_url();

        let mut socket = zeromq::RouterSocket::new();
        socket.bind(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_kernel_control_connection(
        &self,
        session_id: &str,
    ) -> anyhow::Result<KernelControlConnection> {
        let endpoint = self.control_url();

        let mut socket = zeromq::RouterSocket::new();
        socket.bind(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_kernel_stdin_connection(
        &self,
        session_id: &str,
    ) -> anyhow::Result<KernelStdinConnection> {
        let endpoint = self.stdin_url();

        let mut socket = zeromq::RouterSocket::new();
        socket.bind(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_kernel_heartbeat_connection(
        &self,
    ) -> anyhow::Result<KernelHeartbeatConnection> {
        let endpoint = self.hb_url();

        let mut socket = zeromq::RepSocket::new();
        socket.bind(&endpoint).await?;
        anyhow::Ok(KernelHeartbeatConnection { socket })
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_client_iopub_connection(
        &self,
        topic: &str,
        session_id: &str,
    ) -> anyhow::Result<ClientIoPubConnection> {
        let endpoint = self.iopub_url();

        let mut socket = zeromq::SubSocket::new();
        socket.subscribe(topic).await?;

        socket.connect(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_client_shell_connection(
        &self,
        session_id: &str,
    ) -> anyhow::Result<ClientShellConnection> {
        let endpoint = self.shell_url();

        let mut socket = zeromq::DealerSocket::new();
        socket.connect(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_client_control_connection(
        &self,
        session_id: &str,
    ) -> anyhow::Result<ClientControlConnection> {
        let endpoint = self.control_url();

        let mut socket = zeromq::DealerSocket::new();
        socket.connect(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_client_stdin_connection(
        &self,
        session_id: &str,
    ) -> anyhow::Result<ClientStdinConnection> {
        let endpoint = self.stdin_url();

        let mut socket = zeromq::DealerSocket::new();
        socket.connect(&endpoint).await?;
        anyhow::Ok(Connection::new(socket, &self.key, session_id))
    }

    #[cfg(any(feature = "tokio-runtime", feature = "async-dispatcher-runtime"))]
    pub async fn create_client_heartbeat_connection(
        &self,
    ) -> anyhow::Result<ClientHeartbeatConnection> {
        let endpoint = self.hb_url();

        let mut socket = zeromq::ReqSocket::new();
        socket.connect(&endpoint).await?;
        anyhow::Ok(ClientHeartbeatConnection { socket })
    }
}
