use crate::util::{format_bytes, render_progress_bar};
use reqwest::{Client, ClientBuilder, cookie::Jar};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ApiInfo {
    path: String,
    #[allow(dead_code)]
    min_version: u32, // Not used anywhere, so it's OK to use dead_code here
    max_version: u32,
}

#[derive(Debug)]
pub struct SynologyClient {
    base_url: String,
    http: Client,
    sid: Option<String>,
    apis: HashMap<String, ApiInfo>,
}

#[derive(Debug, Deserialize)]
pub struct AuthData {
    sid: String,
}

// Handle error codes
#[derive(Debug, Deserialize)]
pub struct SynologyError {
    code: u32,
}

impl SynologyError {
    fn into_auth_error(self) -> AuthError {
        AuthError::from_code(self.code)
    }
}
// Handle error codes

// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    NoSuchAccount,     // 400
    AccountDisabled,   // 401
    PermissionDenied,  // 402
    TwoStepRequired,   // 403
    TwoStepCodeFailed, // 404
    Other(u32),        // Any other error code, not in the documentation
    ParseError,        // JSON parsing or network error
}

impl AuthError {
    pub fn from_code(code: u32) -> Self {
        match code {
            400 => AuthError::NoSuchAccount,
            401 => AuthError::AccountDisabled,
            402 => AuthError::PermissionDenied,
            403 => AuthError::TwoStepRequired,
            404 => AuthError::TwoStepCodeFailed,
            other => AuthError::Other(other),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AuthError::NoSuchAccount => "No such account or incorrect password",
            AuthError::AccountDisabled => "Account disabled",
            AuthError::PermissionDenied => "Permission denied",
            AuthError::TwoStepRequired => "2-step verification code required",
            AuthError::TwoStepCodeFailed => "Failed to authenticate 2-step verification code",
            AuthError::Other(_) => "Unknown authentication error",
            AuthError::ParseError => "Failed to parse authentication response",
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Other(code) => write!(f, "{} (code {})", self.description(), code),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl std::error::Error for AuthError {}
// Authentication errors

// Task errors
#[derive(Debug)]
pub enum TaskError {
    FileUploadFailed,        // 400
    MaxNumberOfTasksReached, // 401
    DestinationDenied,       // 402
    DestinationDoesNotExist, // 403
    InvalidTaskId,           // 404
    InvalidTaskAction,       // 405
    NoDefaultDestination,    // 406
    SetDestinationFailed,    // 407
    FileDoesNotExist,        // 408
    Other(u32),              // Any other error code, not in documentation
    ParseError,              // JSON parsing error or network error
}

impl TaskError {
    pub fn from_code(code: u32) -> Self {
        match code {
            400 => TaskError::FileUploadFailed,
            401 => TaskError::MaxNumberOfTasksReached,
            402 => TaskError::DestinationDenied,
            403 => TaskError::DestinationDoesNotExist,
            404 => TaskError::InvalidTaskId,
            405 => TaskError::InvalidTaskAction,
            406 => TaskError::NoDefaultDestination,
            407 => TaskError::SetDestinationFailed,
            408 => TaskError::FileDoesNotExist,
            other => TaskError::Other(other),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            TaskError::FileUploadFailed => "File upload failed",
            TaskError::MaxNumberOfTasksReached => "Maximum number of tasks reached",
            TaskError::DestinationDenied => "Destination denied",
            TaskError::DestinationDoesNotExist => "Destination does not exist",
            TaskError::InvalidTaskId => "Invalid task ID",
            TaskError::InvalidTaskAction => "Invalid task action",
            TaskError::NoDefaultDestination => "No default destination configured",
            TaskError::SetDestinationFailed => "Setting destination failed",
            TaskError::FileDoesNotExist => "File does not exist",
            TaskError::Other(_) => "Unknown error",
            TaskError::ParseError => "Failed to parse response",
        }
    }
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::Other(code) => write!(f, "{} (code {})", self.description(), code),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl std::error::Error for TaskError {}
// Task errors

#[derive(Debug, Deserialize)]
pub struct ConfigData {
    pub bt_max_download: u64,
    pub bt_max_upload: u64,
    pub default_destination: Option<String>,
    pub emule_default_destination: Option<String>,
    pub emule_enabled: bool,
    pub emule_max_download: u64,
    pub emule_max_upload: u64,
    pub ftp_max_download: u64,
    pub http_max_download: u64,
    pub nzb_max_download: u64,
    pub unzip_service_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct AuthResponse {
    success: bool,
    data: Option<AuthData>,
    error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigResponse {
    success: bool,
    data: ConfigData,
    #[allow(dead_code)]
    error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
pub struct TaskListResponse {
    success: bool,
    data: Option<TaskData>,
    error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct TaskData {
    offset: usize, // Not used anywhere so it's OK to use dead_code here
    total: usize,  // Not used anywhere so it's OK to use dead_code here
    tasks: Vec<DownloadTask>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadTask {
    pub id: String,
    pub size: u64,
    pub status: TaskStatus,
    pub title: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub username: String,
    pub additional: Option<AdditionalInfo>,
}

#[derive(Debug, Deserialize)]
pub struct AdditionalInfo {
    pub detail: Option<DetailInfo>,
    pub transfer: Option<TransferInfo>,
    pub file: Option<Vec<FileInfo>>,
    pub peer: Option<Vec<PeerInfo>>,
    pub tracker: Option<Vec<TrackerInfo>>,
}

#[derive(Debug, Deserialize)]
pub struct DetailInfo {
    pub connected_leechers: Option<u64>,
    pub connected_seeders: Option<u64>,
    pub connected_peers: Option<u64>,
    pub create_time: Option<u64>,
    pub started_time: Option<u64>,
    pub completed_time: Option<u64>,
    pub seedelapsed: Option<u64>,
    pub destination: Option<String>,
    pub priority: Option<String>,
    pub total_peers: Option<u64>,
    pub total_pieces: Option<u64>,
    pub unzip_password: Option<String>,
    pub waiting_seconds: Option<u64>,
    pub uri: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransferInfo {
    pub size_downloaded: Option<u64>,
    pub size_uploaded: Option<u64>,
    pub downloaded_pieces: Option<u64>,
    pub speed_download: Option<u64>,
    pub speed_upload: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct FileInfo {
    pub filename: Option<String>,
    pub priority: Option<String>,
    pub size: Option<u64>,
    pub size_downloaded: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct TrackerInfo {
    pub url: Option<String>,
    pub status: Option<String>,
    pub update_timer: Option<u64>,
    pub seeds: Option<i64>,
    pub peers: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PeerInfo {
    pub address: Option<String>,
    pub agent: Option<String>,
    pub progress: Option<f64>,
    pub speed_download: Option<u64>,
    pub speed_upload: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct TaskDetailResponse {
    success: bool,
    pub data: Option<TaskInfoData>,
    pub error: Option<SynologyError>,
}

#[derive(Debug, Deserialize)]
pub struct TaskActionResponse {
    pub error: u32,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskActionResponseWrapper {
    pub success: bool,
    pub data: Option<Vec<TaskActionResponse>>,
    pub error: Option<SynologyError>,
}

#[derive(Debug)]
pub struct CreateTaskResponseWrapper {
    pub success: bool,
    pub error: Option<TaskError>,
}

#[derive(Debug, Deserialize)]
pub struct TaskInfoData {
    pub tasks: Vec<DownloadTask>,
}

impl SynologyClient {
    pub fn new(base_url: &str) -> Self {
        let cookie_store = Arc::new(Jar::default());
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .cookie_provider(cookie_store)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            base_url: base_url.to_string(),
            http: client,
            sid: None,
            apis: HashMap::new(),
        }
    }

    pub async fn get_available_apis(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("{}/webapi/query.cgi", self.base_url);
        let params = [
            ("api", "SYNO.API.Info"),
            ("version", "1"),
            ("method", "query"),
            ("query", "ALL"),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let json: serde_json::Value = res.json().await?;

        let data = json
            .get("data")
            .ok_or("Missing `data` field")?
            .as_object()
            .ok_or("`data` is not an object")?;

        for (api_name, info) in data.iter() {
            if let (Some(path), Some(min_version), Some(max_version)) = (
                info.get("path").and_then(|v| v.as_str()),
                info.get("minVersion").and_then(|v| v.as_u64()),
                info.get("maxVersion").and_then(|v| v.as_u64()),
            ) {
                self.apis.insert(
                    api_name.to_string(),
                    ApiInfo {
                        path: path.to_string(),
                        min_version: min_version as u32,
                        max_version: max_version as u32,
                    },
                );
            }
        }

        Ok(())
    }

    pub fn api_info(&self, api: &str) -> Option<&ApiInfo> {
        self.apis.get(api)
    }

    pub fn api_url(&self, api: &str) -> Option<String> {
        self.api_info(api)
            .map(|info| format!("{}/webapi/{}", self.base_url, info.path))
    }

    // Use the max_version of the available APIs
    pub fn api_version(&self, api: &str) -> Option<u32> {
        self.api_info(api).map(|info| info.max_version)
    }

    // Handle login
    pub async fn login(
        &mut self,
        account: &str,
        password: &str,
        session: &str,
    ) -> Result<(), AuthError> {
        let url = self.api_url("SYNO.API.Auth").ok_or(AuthError::Other(0))?;
        let version = self
            .api_version("SYNO.API.Auth")
            .ok_or(AuthError::Other(0))?
            .to_string();

        let params = [
            ("api", "SYNO.API.Auth"),
            ("method", "login"),
            ("version", &version),
            ("account", account),
            ("passwd", password),
            ("session", session),
            ("format", "sid"),
        ];

        let resp = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|_| AuthError::ParseError)?;

        let auth: AuthResponse = resp.json().await.map_err(|_| AuthError::ParseError)?;

        if !auth.success {
            // map the numeric code to our enum
            return Err(match auth.error {
                Some(e) => e.into_auth_error(),
                None => AuthError::Other(0),
            });
        }

        // extract SID
        let sid = auth.data.ok_or(AuthError::Other(0))?.sid;
        self.sid = Some(sid);

        Ok(())
    }

    // Handle logout
    pub async fn logout(&mut self, session: &str) -> Result<(), AuthError> {
        let url = self.api_url("SYNO.API.Auth").ok_or(AuthError::Other(0))?;
        let version = self
            .api_version("SYNO.API.Auth")
            .ok_or(AuthError::Other(0))?
            .to_string();

        let params = [
            ("api", "SYNO.API.Auth"),
            ("method", "logout"),
            ("version", &version),
            ("session", session),
        ];

        let resp = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|_| AuthError::ParseError)?;

        let auth: AuthResponse =
            serde_json::from_str(&resp.text().await.map_err(|_| AuthError::ParseError)?)
                .map_err(|_| AuthError::ParseError)?;

        if !auth.success {
            return Err(match auth.error {
                Some(e) => e.into_auth_error(),
                None => AuthError::Other(0),
            });
        }
        println!("✅ Logged out successfully!");
        Ok(())
    }

    // Use the result of the parsed config data
    pub async fn get_config(&self) -> Result<ConfigData, Box<dyn std::error::Error>> {
        let url = self
            .api_url("SYNO.DownloadStation.Info")
            .ok_or("Missing API info for SYNO.DownloadStation.Info")?;
        let version = self
            .api_version("SYNO.DownloadStation.Info")
            .ok_or("Missing version info for SYNO.DownloadStation.Info")?
            .to_string();
        let sid = self.sid.as_ref().ok_or("Not logged in")?;
        let params = [
            ("api", "SYNO.DownloadStation.Info"),
            ("version", &version),
            ("method", "getconfig"),
            ("_sid", sid),
        ];
        let resp = self.http.get(&url).query(&params).send().await?;
        let text = resp.text().await?;
        let wrapper: ConfigResponse =
            serde_json::from_str(&text).map_err(|e| format!("parsing ConfigResponse: {}", e))?;
        if !wrapper.success {
            Err("getconfig returned success=false".into())
        } else {
            Ok(wrapper.data)
        }
    }

    pub async fn list_download_tasks(
        &self,
    ) -> Result<Vec<DownloadTask>, Box<dyn std::error::Error>> {
        let sid = self.sid.as_ref().ok_or("SID missing — not logged in")?;
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing version info for SYNO.DownloadStation.Task")?
            .to_string();
        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("method", "list"),
            ("version", &version),
            ("_sid", sid), // inject SID manually in case cookies don't work
            ("additional", "detail,transfer,file,peer,tracker"),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        let parsed: TaskListResponse = serde_json::from_str(&body)?;
        if !parsed.success {
            println!("❌ Failed to list tasks: {:?}", parsed.error);
            return Err("Failed to list tasks".into());
        }

        Ok(parsed.data.ok_or("No task data returned")?.tasks)
    }

    pub async fn list_download_task_ids(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let sid = self.sid.as_ref().ok_or("SID is missing - not logged in")?;
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing API version")?
            .to_string();
        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("method", "list"),
            ("version", &version),
            ("_sid", sid),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        let parsed: TaskListResponse = serde_json::from_str(&body)?;

        if parsed.success {
            let task_ids = parsed
                .data
                .unwrap()
                .tasks
                .into_iter()
                .map(|task| task.id)
                .collect();
            Ok(task_ids)
        } else {
            Err(format!("❌ Failed to list tasks: {:?}", parsed.error).into())
        }
    }

    pub async fn get_task_details(
        &self,
        ids: &[String],
    ) -> Result<Vec<DownloadTask>, Box<dyn std::error::Error>> {
        let sid = self.sid.as_ref().ok_or("SID missing — not logged in")?;
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing API version")?
            .to_string();
        let id_list = ids.join(",");
        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("method", "getinfo"),
            ("version", &version),
            ("_sid", sid),
            ("id", &id_list),
            ("additional", "detail,transfer,file,peer,tracker"),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        let parsed: TaskDetailResponse = serde_json::from_str(&body)?;

        if parsed.success {
            Ok(parsed.data.unwrap().tasks)
        } else {
            Err(format!("❌ Failed to fetch task info: {:?}", parsed.error).into())
        }
    }

    pub async fn pause_task(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Build URL & parameters, early‐return on missing API info or SID
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing API info for SYNO.DownloadStation.Task")?;

        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing version info for SYNO.DownloadStation.Task")?
            .to_string();

        let sid = self
            .sid
            .as_ref()
            .ok_or("❌ No session ID (not logged in)")?;

        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("version", &version),
            ("method", "pause"),
            ("id", id),
            ("_sid", sid),
        ];

        // Send request, map any HTTP‐level errors
        let body = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Network error during pause_task: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Failed to read pause response body: {}", e))?;

        // Parse the standard WebAPI envelope
        let wrapper: TaskActionResponseWrapper = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse pause_task envelope: {}", e))?;

        // Check overall success flag
        if !wrapper.success {
            return Err(format!("Pause API returned success=false: {:?}", wrapper.error).into());
        }

        // Unwrap the inner array of per‐task results
        let results = wrapper.data.ok_or("Pause API returned no `data` field")?;

        // Find any non-zero error codes
        if let Some(failed) = results.into_iter().find(|r| r.error != 0) {
            return Err(format!(
                "Pause failed for task {}: error code {}",
                failed.id, failed.error
            )
            .into());
        }

        // Everything OK
        Ok(())
    }

    // Exactly the same envelope‐unwrap logic as pause_task, but calling “resume”
    pub async fn resume_task(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("Missing API info for SYNO.DownloadStation.Task")?;
        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("Missing version info for SYNO.DownloadStation.Task")?
            .to_string();

        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("version", &version),
            ("method", "resume"), // <<< here!
            ("id", id),
            ("_sid", self.sid.as_ref().ok_or("No SID")?),
        ];

        let res = self.http.get(&url).query(&params).send().await?;
        let body = res.text().await?;

        // Parse the same envelope like in pause_task
        let wrapper: TaskActionResponseWrapper = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse resume envelope: {}", e))?;
        if !wrapper.success {
            return Err(format!("Resume API returned success=false: {:?}", wrapper.error).into());
        }
        let responses = wrapper.data.ok_or("Resume API returned no data")?;
        let mut errs = responses
            .into_iter()
            .filter(|r| r.error != 0)
            .map(|r| (r.id, r.error))
            .collect::<Vec<_>>();
        if let Some((bad_id, code)) = errs.pop() {
            Err(format!("Resume failed for {} (code {})", bad_id, code).into())
        } else {
            Ok(())
        }
    }

    pub async fn delete_task(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Build URL & parameters, early‐return on missing API info or SID
        let url = self
            .api_url("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing API info for SYNO.DownloadStation.Task")?;

        let version = self
            .api_version("SYNO.DownloadStation.Task")
            .ok_or("❌ Missing version info for SYNO.DownloadStation.Task")?
            .to_string();

        let sid = self
            .sid
            .as_ref()
            .ok_or("❌ No session ID (not logged in)")?;

        let params = [
            ("api", "SYNO.DownloadStation.Task"),
            ("version", &version),
            ("method", "delete"),
            ("id", id),
            ("_sid", sid),
        ];

        // Send request, map any HTTP‐level errors
        let body = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| format!("Network error during delete_task: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Failed to read delete response body: {}", e))?;

        // Parse the standard WebAPI envelope
        let wrapper: TaskActionResponseWrapper = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse delete_task envelope: {}", e))?;

        // Check overall success flag
        if !wrapper.success {
            return Err(format!("Delete API returned success=false: {:?}", wrapper.error).into());
        }

        // Unwrap the inner array of per‐task results
        let results = wrapper.data.ok_or("Delete API returned no `data` field")?;

        // Find any non-zero error codes
        if let Some(failed) = results.into_iter().find(|r| r.error != 0) {
            return Err(format!(
                "Delete failed for task {}: error code {}",
                failed.id, failed.error
            )
            .into());
        }

        // Everything OK
        Ok(())
    }

    // Add task from URL
    pub async fn create_task_from_url(&self, uri: &str) -> Result<(), TaskError> {
        // Build URL & parameters, early‐return on missing API info or SID
        let api = "SYNO.DownloadStation.Task";
        let endpoint = self.api_url(api).ok_or(TaskError::ParseError)?; // missing API info is unexpected here
        let version = self
            .api_version(api)
            .ok_or(TaskError::ParseError)?
            .to_string();

        let sid = self.sid.as_ref().ok_or(TaskError::ParseError)?;
        let params = [
            ("api", api),
            ("version", &version),
            ("method", "create"),
            ("uri", uri),
            ("_sid", sid),
        ];

        let resp = self
            .http
            .get(&endpoint)
            .query(&params)
            .send()
            .await
            .map_err(|_| TaskError::ParseError)?;
        let text = resp.text().await.map_err(|_| TaskError::ParseError)?;

        #[derive(serde::Deserialize)]
        struct RawResponse {
            success: bool,
            error: Option<ErrorObj>,
        }
        #[derive(serde::Deserialize)]
        struct ErrorObj {
            code: u32,
        }

        let parsed: RawResponse = serde_json::from_str(&text).map_err(|_| TaskError::ParseError)?;

        if parsed.success {
            Ok(())
        } else if let Some(err) = parsed.error {
            Err(TaskError::from_code(err.code))
        } else {
            Err(TaskError::ParseError)
        }
    }
}

impl DownloadTask {
    // Calculate ratio for torrent files
    pub fn upload_download_ratio(&self) -> Option<f64> {
        let additional = self.additional.as_ref()?;
        let transfer = additional.transfer.as_ref()?;
        let downloaded = transfer.size_downloaded?;
        let uploaded = transfer.size_uploaded?;

        if downloaded == 0 {
            return None;
        }

        Some(uploaded as f64 / downloaded as f64)
    }

    // Handle all task types (at least every task type in the old documentation)
    pub fn task_type(&self) -> &str {
        match self.task_type.as_str() {
            "bt" => "Bittorrent",
            "ftp" => "FTP download",
            "http" => "HTTP download",
            "https" => "HTTPS download",
            _ => "Other type of download",
        }
    }

    // Create a Vec from the downloaded info so the table can handle it later
    pub fn to_row_cells(&self) -> Vec<String> {
        let mut cells = Vec::new();

        // Title
        cells.push(self.title.clone());

        // Size
        cells.push(format_bytes(self.size));

        // Downloaded (if available)
        if let Some(add) = &self.additional {
            let downloaded = add
                .transfer
                .as_ref()
                .and_then(|t| t.size_downloaded)
                .unwrap_or(0);
            cells.push(format_bytes(downloaded));
        } else {
            cells.push("-".into());
        }

        // Uploaded
        if let Some(add) = &self.additional {
            let uploaded = add
                .transfer
                .as_ref()
                .and_then(|t| t.size_uploaded)
                .unwrap_or(0);
            cells.push(format_bytes(uploaded));
        } else {
            cells.push("-".into());
        }

        // Progress
        if let Some(add) = &self.additional {
            // first unwrap the TransferInfo
            if let Some(transfer) = &add.transfer {
                // compute percent complete (downloaded vs total size)
                let downloaded = transfer.size_downloaded.unwrap_or(0);
                let total = self.size;
                let pct = if total > 0 {
                    ((downloaded as f64 / total as f64) * 100.0).round() as u64
                } else {
                    0
                };
                // cells.push(format!("{}%", pct));
                cells.push(render_progress_bar(pct));
            } else {
                // no TransferInfo at all
                cells.push("-".into());
            }
        } else {
            cells.push("-".into());
        }

        // Upload speed
        if let Some(add) = &self.additional {
            let upload_speed = add
                .transfer
                .as_ref()
                .and_then(|t| t.speed_upload)
                .unwrap_or(0);
            cells.push(format_bytes(upload_speed));
        } else {
            // no TransferInfo at all
            cells.push("-".into());
        }

        // Download speed
        if let Some(add) = &self.additional {
            let download_speed = add
                .transfer
                .as_ref()
                .and_then(|t| t.speed_download)
                .unwrap_or(0);
            cells.push(format_bytes(download_speed));
        } else {
            // no TransferInfo at all
            cells.push("-".into());
        }

        // Ratio
        if let Some(ratio) = self.upload_download_ratio() {
            cells.push(format!("{:.2}", ratio));
        } else {
            cells.push("-".into());
        }

        // Status label
        cells.push(self.status.label().to_string());

        cells
    }
}

// For some reason the API provides both string and numbers as status... Oh, well...
#[derive(Debug)]
pub enum TaskStatus {
    Code(u64),
    Name(String),
}

// The official API documentation is from 2014 and I'm sure these codes are not perfectly match
// anymore
impl TaskStatus {
    pub fn label(&self) -> &str {
        match self {
            TaskStatus::Name(s) => s,
            TaskStatus::Code(code) => match code {
                1 | 12 => "waiting", // Why?
                2 => "downloading",
                3 => "paused",
                4 => "finishing",
                5 => "finished",
                6 => "hash_checking",
                7 => "filehost_waiting",
                8 => "seeding",
                9 => "extracting",
                10 => "error",
                101 => "duplicate_torrent",
                107 => "timeout",
                _ => "unknown",
            },
        }
    }
}

impl<'de> Deserialize<'de> for TaskStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TaskStatusVisitor;

        impl serde::de::Visitor<'_> for TaskStatusVisitor {
            type Value = TaskStatus;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a string or integer status")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TaskStatus::Name(v.to_string()))
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(TaskStatus::Code(v))
            }
        }

        deserializer.deserialize_any(TaskStatusVisitor)
    }
}
