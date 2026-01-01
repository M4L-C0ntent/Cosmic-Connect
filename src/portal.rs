// src/portal.rs
// Optimized zenity-only file picker (portal backend broken)

/// File filter for the portal file picker
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub patterns: Vec<String>,
}

impl FileFilter {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            patterns: Vec::new(),
        }
    }

    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.patterns.push(pattern.into());
        self
    }

    pub fn patterns(mut self, patterns: Vec<String>) -> Self {
        self.patterns = patterns;
        self
    }
}

// Skip portal - GTK backend can't create windows
const USE_ZENITY_DIRECTLY: bool = true;

/// Open file picker dialog for selecting files
pub async fn pick_files(
    title: impl Into<String>,
    multiple: bool,
    filters: Option<Vec<FileFilter>>,
) -> Vec<String> {
    let title_str = title.into();
    
    if USE_ZENITY_DIRECTLY {
        return pick_files_zenity(title_str, multiple, filters).await;
    }
    
    // Portal code would go here
    Vec::new()
}

/// Open folder picker dialog for selecting a directory
pub async fn pick_folder(title: impl Into<String>) -> Option<String> {
    let title_str = title.into();
    
    if USE_ZENITY_DIRECTLY {
        return pick_folder_zenity(title_str).await;
    }
    
    // Portal code would go here
    None
}

/// Pick files using zenity
async fn pick_files_zenity(
    title: String,
    multiple: bool,
    filters: Option<Vec<FileFilter>>,
) -> Vec<String> {
    let mut args = vec!["--file-selection".to_string()];
    args.push(format!("--title={}", title));
    
    if multiple {
        args.push("--multiple".to_string());
        args.push("--separator=|".to_string());
    }
    
    // Add filters
    if let Some(filter_list) = filters {
        for filter in filter_list {
            if !filter.patterns.is_empty() {
                let filter_str = format!("{} | {}", filter.name, filter.patterns.join(" "));
                args.push(format!("--file-filter={}", filter_str));
            }
        }
    }
    
    match tokio::process::Command::new("zenity")
        .args(&args)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            if let Ok(result) = String::from_utf8(output.stdout) {
                let files: Vec<String> = result
                    .trim()
                    .split('|')
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                return files;
            }
        }
        Ok(_) => {}
        Err(_) => {}
    }
    
    Vec::new()
}

/// Pick folder using zenity
async fn pick_folder_zenity(title: String) -> Option<String> {
    let args = vec![
        "--file-selection".to_string(),
        "--directory".to_string(),
        format!("--title={}", title),
    ];
    
    match tokio::process::Command::new("zenity")
        .args(&args)
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            if let Ok(result) = String::from_utf8(output.stdout) {
                let folder = result.trim().to_string();
                
                if !folder.is_empty() {
                    return Some(folder);
                }
            }
        }
        Ok(_) => {}
        Err(_) => {}
    }
    
    None
}