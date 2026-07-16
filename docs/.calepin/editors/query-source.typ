#import "/.calepin/calepin.typ" as calepin

#set document(title: [Editor integration])
#metadata((tags: ("getting started", "editors", "VS Code", "Tinymist"))) <website-metadata>

#title()

Calepin documents are ordinary Typst files, so you can keep your editor's language tooling and preview while Calepin evaluates computational chunks.

The _Calepin for Typst_ extension adds start and stop commands for Calepin in VS Code, Cursor, Positron, and other VSX-compatible editors. This short screencast shows the workflow alongside Tinymist preview.

#calepin.elements.lightbox-video(
  "vscode-calepin-screencast",
  "/assets/calepin_vscode.mp4",
  poster: "/assets/calepin_vscode-thumb.png",
  width: 48em,
)
