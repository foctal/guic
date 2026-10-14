use crate::AppPaths;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const MAX_REQUEST: usize = 64 * 1024;

/// A launch request retained until the application's UI is ready to consume it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActivationRequest {
    /// Original arguments, excluding the executable, preserving native encoding.
    pub arguments: Vec<OsString>,
    /// Working directory of the launching process, for resolving relative files.
    #[serde(with = "native_path")]
    pub working_directory: PathBuf,
    /// URLs delivered by the operating system.
    pub urls: Vec<String>,
    /// Whether an existing application should be made visible.
    pub reopen: bool,
}

impl ActivationRequest {
    /// Captures the initial process arguments and working directory.
    pub fn current() -> std::io::Result<Self> {
        Ok(Self {
            arguments: std::env::args_os().skip(1).collect(),
            working_directory: std::env::current_dir()?,
            urls: Vec::new(),
            reopen: true,
        })
    }

    /// Resolves a file argument against the launching process's directory.
    /// The host selects file arguments according to its own command-line grammar.
    pub fn file_argument(&self, argument: impl AsRef<std::path::Path>) -> PathBuf {
        self.working_directory.join(argument)
    }
    /// Converts OS file URLs to local filesystem paths, rejecting non-file URLs.
    pub fn files(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.urls
            .iter()
            .filter_map(|value| url::Url::parse(value).ok()?.to_file_path().ok())
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.working_directory.is_absolute(),
            "activation working directory must be absolute"
        );
        for value in &self.urls {
            let _ = url::Url::parse(value).context("invalid activation URL")?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    port: u16,
    token: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Packet {
    token: String,
    id: String,
    request: ActivationRequest,
}

/// Result of acquiring the per-user application instance.
pub enum Instance {
    /// Owns activation delivery. Retain this owner for the application lifetime.
    Primary(ActivationOwner),
    /// The primary process acknowledged the request; the caller should exit.
    Forwarded,
}

/// Primary-instance lock and authenticated local activation receiver.
///
/// A bounded queue retains cold-start and secondary-launch requests. Forwarding
/// succeeds only after a request is accepted by the queue. Dropping the owner
/// stops its listener, removes its endpoint, and releases the lock.
pub struct ActivationOwner {
    _lock: File,
    endpoint: PathBuf,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    sender: async_channel::Sender<ActivationRequest>,
    receiver: async_channel::Receiver<ActivationRequest>,
}

impl Instance {
    /// Acquires an instance or forwards to it, waiting briefly for primary startup.
    /// Call before launching the GUI event loop. Identity is scoped by `AppPaths`.
    pub fn acquire(paths: &AppPaths, request: ActivationRequest) -> Result<Self> {
        request.validate()?;
        ensure!(
            serde_json::to_vec(&request)?.len() + 160 <= MAX_REQUEST,
            "activation request is too large"
        );
        let directory = paths.local_data.join("instance");
        std::fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("owner.lock"))?;
        let endpoint_path = directory.join("endpoint.json");
        let started = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();
        loop {
            match lock.try_lock_exclusive() {
                Ok(()) => {
                    return ActivationOwner::start(lock, endpoint_path, request).map(Self::Primary);
                }
                Err(error)
                    if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {}
                Err(error) => return Err(error.into()),
            }
            if let Ok(bytes) = std::fs::read(&endpoint_path)
                && let Ok(endpoint) = serde_json::from_slice::<Endpoint>(&bytes)
                && forward(&endpoint, &request_id, &request).is_ok()
            {
                return Ok(Self::Forwarded);
            }
            ensure!(
                started.elapsed() < Duration::from_secs(5),
                "primary instance did not acknowledge activation"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

fn forward(endpoint: &Endpoint, id: &str, request: &ActivationRequest) -> Result<()> {
    let packet = serde_json::to_vec(&Packet {
        token: endpoint.token.clone(),
        id: id.into(),
        request: request.clone(),
    })?;
    ensure!(
        packet.len() <= MAX_REQUEST,
        "activation request is too large"
    );
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(250))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    stream.write_all(&(packet.len() as u32).to_be_bytes())?;
    stream.write_all(&packet)?;
    let mut ack = [0];
    stream.read_exact(&mut ack)?;
    ensure!(ack == [1], "activation queue is unavailable");
    Ok(())
}

impl ActivationOwner {
    fn start(lock: File, endpoint_path: PathBuf, request: ActivationRequest) -> Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let endpoint = Endpoint {
            port: listener.local_addr()?.port(),
            token: uuid::Uuid::new_v4().to_string(),
        };
        let packet = serde_json::to_vec(&Packet {
            token: endpoint.token.clone(),
            id: uuid::Uuid::new_v4().to_string(),
            request: request.clone(),
        })?;
        ensure!(
            packet.len() <= MAX_REQUEST,
            "activation request is too large"
        );
        // The lock makes stale endpoint removal safe after an interrupted process.
        match std::fs::remove_file(&endpoint_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&endpoint_path)?;
        file.write_all(&serde_json::to_vec(&endpoint)?)?;
        file.sync_all()?;
        let (sender, receiver) = async_channel::bounded(64);
        sender.try_send(request)?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker_sender = sender.clone();
        let worker = std::thread::Builder::new()
            .name("guic-activation".into())
            .spawn(move || {
                let mut delivered = std::collections::VecDeque::new();
                while !worker_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let result = receive(
                                &mut stream,
                                &endpoint.token,
                                &worker_sender,
                                &mut delivered,
                            );
                            if result.is_err() {
                                let _ = stream.write_all(&[0]);
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(20))
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(Self {
            _lock: lock,
            endpoint: endpoint_path,
            stop,
            worker: Some(worker),
            sender,
            receiver,
        })
    }

    /// Returns a clone of the queue receiver. Receivers compete, not broadcast.
    pub fn receiver(&self) -> async_channel::Receiver<ActivationRequest> {
        self.receiver.clone()
    }

    /// Connects OS URL and reopen callbacks before `Application::run`.
    /// Install this once; GPUI has one callback slot for each event.
    pub fn bind(&self, app: &gpui::Application) {
        let sender = self.sender.clone();
        app.on_open_urls(move |urls| {
            let request = ActivationRequest {
                arguments: Vec::new(),
                working_directory: std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()),
                urls,
                reopen: true,
            };
            if let Err(error) = request
                .validate()
                .and_then(|_| sender.try_send(request).map_err(Into::into))
            {
                tracing::error!("could not queue OS activation: {error}");
            }
        });
        let sender = self.sender.clone();
        app.on_reopen(move |_| {
            let request = ActivationRequest {
                arguments: Vec::new(),
                working_directory: std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir()),
                urls: Vec::new(),
                reopen: true,
            };
            if let Err(error) = sender.try_send(request) {
                tracing::error!("could not queue reopen: {error}");
            }
        });
    }

    /// Delivers queued requests on the GPUI thread, starting when the host is ready.
    /// The callback owns file/URL routing; the active or first window is then raised.
    pub fn dispatch(
        &self,
        cx: &mut gpui::App,
        mut handler: impl FnMut(ActivationRequest, &mut gpui::App) + 'static,
    ) -> gpui::Task<()> {
        let receiver = self.receiver();
        cx.spawn(async move |cx| {
            while let Ok(request) = receiver.recv().await {
                cx.update(|cx| {
                    let reopen = request.reopen;
                    handler(request, cx);
                    if reopen {
                        activate_window(cx);
                    }
                });
            }
        })
    }
}

/// Raises the active window, or the first available window after a tray reopen.
pub fn activate_window(cx: &mut gpui::App) {
    if let Some(handle) = cx.active_window().or_else(|| cx.windows().first().copied()) {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
    cx.activate(true);
}

fn receive(
    stream: &mut TcpStream,
    token: &str,
    sender: &async_channel::Sender<ActivationRequest>,
    delivered: &mut std::collections::VecDeque<String>,
) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    ensure!(length <= MAX_REQUEST, "activation request is too large");
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    let packet: Packet = serde_json::from_slice(&bytes)?;
    ensure!(packet.token == token, "invalid activation token");
    packet.request.validate()?;
    ensure!(
        uuid::Uuid::parse_str(&packet.id).is_ok(),
        "invalid activation request id"
    );
    if !delivered.contains(&packet.id) {
        sender.try_send(packet.request)?;
        delivered.push_back(packet.id);
        if delivered.len() > 256 {
            delivered.pop_front();
        }
    }
    stream.write_all(&[1])?;
    Ok(())
}

impl Drop for ActivationOwner {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.sender.close();
        let _ = std::fs::remove_file(&self.endpoint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_native_encoding_survives_forwarding() -> Result<()> {
        use std::os::windows::ffi::OsStringExt;
        let directory = tempfile::tempdir()?;
        let paths = AppPaths::under(
            directory.path(),
            &crate::AppIdentity::new("org.example.windows", "Windows", "1")?,
        )?;
        let request = ActivationRequest {
            arguments: vec![
                OsString::from_wide(&[0xD800, 0x20, 0x65E5]),
                "quote\" and spaces\\".into(),
            ],
            working_directory: PathBuf::from(OsString::from_wide(&[0x43, 0x3A, 0x5C, 0xDFFF])),
            urls: vec!["guic-example://open/path?q=hello%20world".into()],
            reopen: true,
        };
        let Instance::Primary(owner) = Instance::acquire(&paths, request.clone())? else {
            anyhow::bail!("expected primary")
        };
        assert!(matches!(
            Instance::acquire(&paths, request.clone())?,
            Instance::Forwarded
        ));
        assert_eq!(owner.receiver.try_recv()?, request);
        assert_eq!(owner.receiver.try_recv()?, request);
        Ok(())
    }

    #[test]
    fn simultaneous_launches_elect_one_owner_and_recover_stale_endpoint() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let paths = AppPaths::under(
            directory.path(),
            &crate::AppIdentity::new("org.example.race", "Race", "1")?,
        )?;
        let endpoint = paths.local_data.join("instance/endpoint.json");
        std::fs::create_dir_all(endpoint.parent().expect("endpoint directory"))?;
        std::fs::write(&endpoint, "interrupted endpoint")?;
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|index| {
                let paths = paths.clone();
                let barrier = barrier.clone();
                let cwd = directory.path().to_owned();
                std::thread::spawn(move || {
                    barrier.wait();
                    Instance::acquire(
                        &paths,
                        ActivationRequest {
                            arguments: vec![index.to_string().into()],
                            working_directory: cwd,
                            urls: vec![],
                            reopen: true,
                        },
                    )
                })
            })
            .collect();
        let mut owners = Vec::new();
        for worker in workers {
            if let Instance::Primary(owner) = worker.join().expect("launch worker")? {
                owners.push(owner);
            }
        }
        assert_eq!(owners.len(), 1);
        let mut delivered = std::collections::BTreeSet::new();
        while let Ok(request) = owners[0].receiver.try_recv() {
            delivered.insert(request.arguments);
        }
        assert_eq!(delivered.len(), 8);
        drop(owners);
        assert!(!endpoint.exists());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn native_paths_round_trip_without_unicode_conversion() -> Result<()> {
        use std::os::unix::ffi::OsStringExt;
        let request = ActivationRequest {
            arguments: vec![OsString::from_vec(vec![0xff, b'a'])],
            working_directory: PathBuf::from(OsString::from_vec(vec![b'/', 0xfe])),
            urls: vec![],
            reopen: false,
        };
        let encoded = serde_json::to_vec(&request)?;
        assert_eq!(
            serde_json::from_slice::<ActivationRequest>(&encoded)?,
            request
        );
        Ok(())
    }
    #[test]
    fn authentication_and_duplicate_acknowledgements_preserve_exactly_one_delivery() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let paths = AppPaths::under(
            directory.path(),
            &crate::AppIdentity::new("org.example.packet", "Packet", "1")?,
        )?;
        let request = ActivationRequest {
            arguments: vec![],
            working_directory: directory.path().into(),
            urls: vec![],
            reopen: false,
        };
        let Instance::Primary(owner) = Instance::acquire(&paths, request.clone())? else {
            anyhow::bail!("primary")
        };
        owner.receiver.try_recv()?;
        let endpoint: Endpoint = serde_json::from_slice(&std::fs::read(&owner.endpoint)?)?;
        let id = uuid::Uuid::new_v4().to_string();
        assert!(
            forward(
                &Endpoint {
                    port: endpoint.port,
                    token: "wrong".into()
                },
                &id,
                &request
            )
            .is_err()
        );
        assert!(owner.receiver.try_recv().is_err());
        forward(&endpoint, &id, &request)?;
        forward(&endpoint, &id, &request)?;
        assert_eq!(owner.receiver.try_recv()?, request);
        assert!(owner.receiver.try_recv().is_err());
        let oversized = ActivationRequest {
            arguments: vec!["x".repeat(MAX_REQUEST).into()],
            ..request
        };
        assert!(Instance::acquire(&paths, oversized).is_err());
        Ok(())
    }
    #[test]
    fn forwarding_preserves_requests_and_releases_ownership() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let identity = crate::AppIdentity::new("org.example.activation", "Test", "1")?;
        let paths = AppPaths::under(dir.path(), &identity)?;
        let initial = ActivationRequest {
            arguments: vec!["document with spaces".into()],
            working_directory: dir.path().to_owned(),
            urls: vec![],
            reopen: true,
        };
        let Instance::Primary(owner) = Instance::acquire(&paths, initial.clone())? else {
            anyhow::bail!("expected primary")
        };
        let next = ActivationRequest {
            urls: vec!["test://open/item".into()],
            ..initial.clone()
        };
        assert!(matches!(
            Instance::acquire(&paths, next.clone())?,
            Instance::Forwarded
        ));
        assert_eq!(owner.receiver.try_recv()?, initial);
        assert_eq!(owner.receiver.try_recv()?, next);
        drop(owner);
        assert!(matches!(
            Instance::acquire(&paths, initial)?,
            Instance::Primary(_)
        ));
        Ok(())
    }
}

mod native_path {
    use serde::{Deserialize, Serialize};
    pub fn serialize<S: serde::Serializer>(
        path: &std::path::Path,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_os_str().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<std::path::PathBuf, D::Error> {
        Ok(std::ffi::OsString::deserialize(deserializer)?.into())
    }
}

/// Thread-safe implementation of the existing single-instance service contract.
/// Prefer `Instance::acquire` for startup; this adapter is useful for dependency
/// injection. Acquire once, await the result, then bind or consume the owner.
#[derive(Clone)]
pub struct SingleInstanceAdapter {
    paths: AppPaths,
    owner: Arc<std::sync::Mutex<Option<ActivationOwner>>>,
}
impl SingleInstanceAdapter {
    /// Creates an adapter without acquiring a process lock.
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            owner: Arc::default(),
        }
    }
    /// Transfers primary ownership to the application after successful acquisition.
    pub fn take_owner(&self) -> Option<ActivationOwner> {
        self.owner.lock().expect("instance lock poisoned").take()
    }
}
impl guic_core::SingleInstanceService for SingleInstanceAdapter {
    fn acquire_or_forward(
        &self,
        arguments: Vec<String>,
    ) -> guic_core::ServiceFuture<guic_core::InstanceDisposition> {
        let adapter = self.clone();
        let (sender, receiver) = async_channel::bounded(1);
        let cwd = std::env::current_dir();
        let started = std::thread::Builder::new()
            .name("guic-instance-acquire".into())
            .spawn(move || {
                let result = (|| -> Result<_> {
                    let mut owner = adapter
                        .owner
                        .lock()
                        .map_err(|_| anyhow::anyhow!("instance lock poisoned"))?;
                    let request = ActivationRequest {
                        arguments: arguments.into_iter().map(Into::into).collect(),
                        working_directory: cwd?,
                        urls: vec![],
                        reopen: true,
                    };
                    match Instance::acquire(&adapter.paths, request)? {
                        Instance::Primary(primary) => {
                            *owner = Some(primary);
                            Ok(guic_core::InstanceDisposition::Primary)
                        }
                        Instance::Forwarded => Ok(guic_core::InstanceDisposition::Forwarded),
                    }
                })()
                .map_err(|error| {
                    guic_core::ServiceError::new(
                        guic_core::ServiceErrorKind::Platform,
                        error.to_string(),
                    )
                });
                let _ = sender.try_send(result);
            });
        Box::pin(async move {
            started.map_err(|error| {
                guic_core::ServiceError::new(
                    guic_core::ServiceErrorKind::Unavailable,
                    error.to_string(),
                )
            })?;
            receiver.recv().await.map_err(|_| {
                guic_core::ServiceError::new(
                    guic_core::ServiceErrorKind::Unavailable,
                    "instance acquisition stopped",
                )
            })?
        })
    }
}
