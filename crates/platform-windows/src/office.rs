use platform_contract::{ActionKind, ApprovedAction, PlatformError, SemanticElement};
use serde_json::Value;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeApp {
    Excel,
    Word,
    PowerPoint,
}

pub fn detect_office_app(window_title: &str) -> Option<OfficeApp> {
    let lower = window_title.to_lowercase();
    if lower.contains("excel") {
        Some(OfficeApp::Excel)
    } else if lower.contains("word") {
        Some(OfficeApp::Word)
    } else if lower.contains("powerpoint") {
        Some(OfficeApp::PowerPoint)
    } else {
        None
    }
}

pub fn is_office_locator(locator: &str) -> bool {
    locator.starts_with("office:")
}

pub fn office_virtual_elements(app: OfficeApp) -> Vec<SemanticElement> {
    let primary = match app {
        OfficeApp::Excel => SemanticElement {
            locator: "office:excel:range".into(),
            role: "office:excel_grid".into(),
            label: "Active Excel Sheet (use office:excel:A1 or office:excel:A1:C5)".into(),
            bounds: None,
            is_secret: false,
            is_enabled: true,
        },
        OfficeApp::Word => SemanticElement {
            locator: "office:word:body".into(),
            role: "office:word_document".into(),
            label: "Active Word Document Body".into(),
            bounds: None,
            is_secret: false,
            is_enabled: true,
        },
        OfficeApp::PowerPoint => SemanticElement {
            locator: "office:ppt:slide".into(),
            role: "office:ppt_deck".into(),
            label: "Active PowerPoint Presentation (add/update slide)".into(),
            bounds: None,
            is_secret: false,
            is_enabled: true,
        },
    };
    vec![
        primary,
        SemanticElement {
            locator: "office:save".into(),
            role: "office:save_button".into(),
            label: "Save active Office document".into(),
            bounds: None,
            is_secret: false,
            is_enabled: true,
        },
    ]
}

fn run_ps(script: &str, env_json: Option<&str>) -> Result<String, PlatformError> {
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    if let Some(payload) = env_json {
        cmd.env("SNIPPY_OFFICE_PAYLOAD", payload);
    }
    let output = cmd
        .output()
        .map_err(|e| PlatformError::UnsupportedControl(format!("Office COM launch failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(PlatformError::UnsupportedControl(format!(
            "Office COM error: {}",
            if stderr.is_empty() {
                "unknown COM failure".into()
            } else {
                stderr
            }
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn observe_office_summary(app: OfficeApp) -> Option<String> {
    let script = match app {
        OfficeApp::Excel => {
            r#"
$xl = [Runtime.InteropServices.Marshal]::GetActiveObject('Excel.Application')
$wb = $xl.ActiveWorkbook
$ws = $xl.ActiveSheet
$used = $ws.UsedRange.Address($false,$false)
$sel = $xl.Selection.Address($false,$false)
Write-Output ("Excel Workbook: " + $wb.Name + " | Sheet: " + $ws.Name + " | Used: " + $used + " | Selection: " + $sel)
"#
        }
        OfficeApp::Word => {
            r#"
$wd = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$doc = $wd.ActiveDocument
Write-Output ("Word Document: " + $doc.Name + " | Paragraphs: " + $doc.Paragraphs.Count)
"#
        }
        OfficeApp::PowerPoint => {
            r#"
$pp = [Runtime.InteropServices.Marshal]::GetActiveObject('PowerPoint.Application')
$pres = $pp.ActivePresentation
Write-Output ("PowerPoint: " + $pres.Name + " | Slides: " + $pres.Slides.Count)
"#
        }
    };
    run_ps(script, None).ok().filter(|s| !s.is_empty())
}

pub fn execute_office_action(action: &ApprovedAction) -> Result<String, PlatformError> {
    let locator = action
        .target
        .semantic_locator
        .as_deref()
        .ok_or_else(|| PlatformError::StaleLocator("office locator required".into()))?;

    if locator == "office:save" && action.kind == ActionKind::Invoke {
        let script = r#"
foreach ($prog in @('Excel.Application','Word.Application','PowerPoint.Application')) {
  try {
    $app = [Runtime.InteropServices.Marshal]::GetActiveObject($prog)
    if ($prog -eq 'Excel.Application' -and $app.ActiveWorkbook) { $app.ActiveWorkbook.Save(); Write-Output "Saved Excel workbook"; exit 0 }
    if ($prog -eq 'Word.Application' -and $app.ActiveDocument) { $app.ActiveDocument.Save(); Write-Output "Saved Word document"; exit 0 }
    if ($prog -eq 'PowerPoint.Application' -and $app.ActivePresentation) { $app.ActivePresentation.Save(); Write-Output "Saved PowerPoint presentation"; exit 0 }
  } catch {}
}
throw "No active Office document found to save"
"#;
        return run_ps(script, None);
    }

    if let Some(range_part) = locator.strip_prefix("office:excel:") {
        let range = if range_part == "range" {
            action
                .arguments
                .get("range")
                .and_then(|v| v.as_str())
                .unwrap_or("A1")
        } else {
            range_part
        };

        let payload = if let Some(rows) = action.arguments.get("rows") {
            serde_json::json!({ "range": range, "rows": rows })
        } else {
            let text = action
                .arguments
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            serde_json::json!({ "range": range, "rows": [[text]] })
        };

        let script = r#"
$data = $env:SNIPPY_OFFICE_PAYLOAD | ConvertFrom-Json
$xl = [Runtime.InteropServices.Marshal]::GetActiveObject('Excel.Application')
$ws = $xl.ActiveSheet
$startCell = $ws.Range($data.range).Cells.Item(1,1)
$rIdx = 0
foreach ($row in $data.rows) {
  $cIdx = 0
  foreach ($val in $row) {
    $ws.Cells.Item($startCell.Row + $rIdx, $startCell.Column + $cIdx).Value2 = $val
    $cIdx++
  }
  $rIdx++
}
Write-Output ("Updated Excel starting at " + $data.range + " (" + $rIdx + " rows)")
"#;
        return run_ps(script, Some(&payload.to_string()));
    }

    if locator == "office:word:body" {
        let text = action
            .arguments
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PlatformError::UnsupportedControl("missing arguments.text".into()))?;
        let payload = serde_json::json!({ "text": text });
        let script = r#"
$data = $env:SNIPPY_OFFICE_PAYLOAD | ConvertFrom-Json
$wd = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$doc = $wd.ActiveDocument
$rng = $doc.Content
$rng.Collapse(0)
$rng.InsertAfter($data.text + "`r`n")
Write-Output "Inserted text into Word document"
"#;
        return run_ps(script, Some(&payload.to_string()));
    }

    if locator == "office:ppt:slide" {
        let title = action
            .arguments
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Slide");
        let body = action
            .arguments
            .get("body")
            .and_then(|v| v.as_str())
            .or_else(|| action.arguments.get("text").and_then(|v| v.as_str()))
            .unwrap_or("");
        let payload = Value::Object(
            [
                ("title".to_string(), Value::String(title.to_string())),
                ("body".to_string(), Value::String(body.to_string())),
            ]
            .into_iter()
            .collect(),
        );
        let script = r#"
$data = $env:SNIPPY_OFFICE_PAYLOAD | ConvertFrom-Json
$pp = [Runtime.InteropServices.Marshal]::GetActiveObject('PowerPoint.Application')
$pres = $pp.ActivePresentation
$idx = $pres.Slides.Count + 1
$slide = $pres.Slides.Add($idx, 2)
if ($slide.Shapes.HasTitle) { $slide.Shapes.Title.TextFrame.TextRange.Text = $data.title }
if ($slide.Shapes.Count -ge 2) { $slide.Shapes.Item(2).TextFrame.TextRange.Text = $data.body }
Write-Output ("Added PowerPoint slide #" + $idx)
"#;
        return run_ps(script, Some(&payload.to_string()));
    }

    Err(PlatformError::StaleLocator(format!(
        "unknown office locator: {locator}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_office_apps_and_virtual_locators() {
        assert_eq!(detect_office_app("Book1 - Excel"), Some(OfficeApp::Excel));
        assert_eq!(
            detect_office_app("Report.docx - Word"),
            Some(OfficeApp::Word)
        );
        assert_eq!(
            detect_office_app("Deck.pptx - PowerPoint"),
            Some(OfficeApp::PowerPoint)
        );
        assert_eq!(detect_office_app("Google Chrome"), None);
        assert!(is_office_locator("office:excel:A1:C4"));
        assert!(is_office_locator("office:save"));
    }
}
