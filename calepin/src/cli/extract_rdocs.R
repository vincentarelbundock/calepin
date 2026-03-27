pkg <- commandArgs(trailingOnly = TRUE)[1]
outdir <- commandArgs(trailingOnly = TRUE)[2]

# Recursively serialize an Rd node to a JSON-friendly list
rd_to_json <- function(node) {
  tag <- attr(node, "Rd_tag")

  # Leaf text nodes
  if (!is.null(tag) && tag %in% c("TEXT", "RCODE", "VERB", "COMMENT")) {
    return(list(tag = tag, text = paste(as.character(node), collapse = "")))
  }

  # Tagged nodes with children (e.g., \code, \itemize, \description, ...)
  if (!is.null(tag)) {
    children <- lapply(node, rd_to_json)
    return(list(tag = tag, children = children))
  }

  # Untagged list (e.g., \item positional args in \arguments) -- wrap as group
  if (is.list(node)) {
    children <- lapply(node, rd_to_json)
    return(list(tag = "GROUP", children = children))
  }

  # Plain character
  list(tag = "TEXT", text = paste(as.character(node), collapse = ""))
}

# Check if pkg is a directory path
if (dir.exists(pkg)) {
  # Source directory: read .Rd files from man/ subdirectory
  man_dir <- file.path(pkg, "man")
  if (!dir.exists(man_dir)) {
    stop(paste0("No man/ subdirectory found in '", pkg, "'."), call. = FALSE)
  }

  rd_files <- list.files(man_dir, pattern = "\\.Rd$", full.names = TRUE)
  if (length(rd_files) == 0) {
    stop(paste0("No .Rd files found in '", man_dir, "'."), call. = FALSE)
  }

  result <- lapply(rd_files, function(f) {
    rd <- tools::parse_Rd(f)
    topic <- sub("\\.Rd$", "", basename(f))
    nodes <- lapply(rd, rd_to_json)
    list(topic = topic, nodes = nodes)
  })
  result <- result[order(sapply(result, function(x) x$topic))]
} else {
  # Installed package
  if (!requireNamespace(pkg, quietly = TRUE)) {
    stop(paste0("Package '", pkg, "' is not installed."), call. = FALSE)
  }

  help_dir <- system.file("help", package = pkg)
  if (!nzchar(help_dir)) {
    stop(paste0("No help directory found for '", pkg, "'."), call. = FALSE)
  }

  rdb <- tools:::fetchRdDB(file.path(help_dir, pkg))

  topics <- sort(names(rdb))
  result <- lapply(topics, function(topic) {
    rd <- rdb[[topic]]
    nodes <- lapply(rd, rd_to_json)
    list(topic = topic, nodes = nodes)
  })
}

cat(jsonlite::toJSON(result, auto_unbox = TRUE, pretty = FALSE))
