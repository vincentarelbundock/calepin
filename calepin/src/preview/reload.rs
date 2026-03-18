const RELOAD_SCRIPT: &str = r#"<script>
(function() {
  var lastVersion = document.body.dataset.version || "0";
  setInterval(function() {
    fetch('/__version').then(function(r) { return r.text(); }).then(function(v) {
      if (v !== lastVersion) { location.reload(); }
    }).catch(function() {});
  }, 100);
})();
</script>"#;

/// Inject the live-reload polling script before </body>.
/// Also adds a data-version attribute to <body> for the initial version.
pub fn inject_reload_script(html: &str, version: u64) -> String {
    // Add version to <body> — handles both <body> and <body class="...">
    let html = if let Some(pos) = html.find("<body") {
        let rest = &html[pos..];
        let close = rest.find('>').unwrap_or(5);
        format!(
            "{}<body data-version=\"{}\"{}{}",
            &html[..pos],
            version,
            &rest[5..close], // attributes between <body and >
            &rest[close..]   // > and everything after
        )
    } else {
        html.to_string()
    };

    // Inject script before </body>
    if let Some(pos) = html.rfind("</body>") {
        let mut out = String::with_capacity(html.len() + RELOAD_SCRIPT.len() + 1);
        out.push_str(&html[..pos]);
        out.push_str(RELOAD_SCRIPT);
        out.push('\n');
        out.push_str(&html[pos..]);
        out
    } else {
        format!("{}\n{}", html, RELOAD_SCRIPT)
    }
}
