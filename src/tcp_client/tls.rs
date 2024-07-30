use std::sync::Arc;

use my_tls::tokio_rustls::client::TlsStream;
use my_tls::tokio_rustls::rustls::pki_types::ServerName;
use my_tls::ROOT_CERT_STORE;
use tokio::net::TcpStream;

pub async fn do_handshake(
    tcp_client_name: &str,
    tcp_stream: TcpStream,
    server_name: String,
) -> Result<TlsStream<TcpStream>, String> {
    let config = my_tls::tokio_rustls::rustls::ClientConfig::builder()
        .with_root_certificates(ROOT_CERT_STORE.clone())
        .with_no_client_auth();

    let connector = my_tls::tokio_rustls::TlsConnector::from(Arc::new(config));
    let domain = ServerName::try_from(server_name);

    if let Err(err) = &domain {
        return Err(format!(
            "TcpClient:{}.  Something wrong with server name to create TLS Handshake: {:?}",
            tcp_client_name, err
        ));
    }

    let domain = domain.unwrap();

    let tls_stream = connector
        .connect_with(domain, tcp_stream, |itm| {
            println!("Debugging: {:?}", itm.alpn_protocol());
        })
        .await;

    if let Err(err) = &tls_stream {
        return Err(format!(
            "TcpClient: {}. TLS Handshake failed: {:?}",
            tcp_client_name, err
        ));
    }

    Ok(tls_stream.unwrap())
}
