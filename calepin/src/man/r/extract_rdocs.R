pkg <- commandArgs(trailingOnly = TRUE)[1]
outdir <- commandArgs(trailingOnly = TRUE)[2]

if (!requireNamespace(pkg, quietly = TRUE)) {
  stop(paste0("Package '", pkg, "' is not installed."), call. = FALSE)
}

help_dir <- system.file("help", package = pkg)
if (!nzchar(help_dir)) {
  stop(paste0("No help directory found for '", pkg, "'."), call. = FALSE)
}

rdb <- tools:::fetchRdDB(file.path(help_dir, pkg))

# Track referenced external packages
linked_packages <- character()

# Recursively serialize an Rd node to a JSON-friendly list
rd_to_json <- function(node) {
  tag <- attr(node, "Rd_tag")
  option <- attr(node, "Rd_option")

  # Leaf text nodes
  if (!is.null(tag) && tag %in% c("TEXT", "RCODE", "VERB", "COMMENT")) {
    return(list(tag = tag, text = paste(as.character(node), collapse = "")))
  }

  # Tagged nodes with children (e.g., \code, \itemize, \description, ...)
  if (!is.null(tag)) {
    children <- lapply(node, rd_to_json)
    result <- list(tag = tag, children = children)
    if (!is.null(option)) {
      opt_str <- paste(as.character(option), collapse = "")
      result$option <- opt_str
      # Track external package references from \link[pkg]{...}
      if (tag == "\\link" && nzchar(opt_str) && !startsWith(opt_str, "=")) {
        ext_pkg <- sub(":.*", "", opt_str)
        if (ext_pkg != pkg) {
          linked_packages <<- c(linked_packages, ext_pkg)
        }
      }
    }
    return(result)
  }

  # Untagged list (e.g., \item positional args in \arguments) -- wrap as group
  if (is.list(node)) {
    children <- lapply(node, rd_to_json)
    return(list(tag = "GROUP", children = children))
  }

  # Plain character
  list(tag = "TEXT", text = paste(as.character(node), collapse = ""))
}

topics <- sort(names(rdb))
result <- lapply(topics, function(topic) {
  rd <- rdb[[topic]]
  nodes <- lapply(rd, rd_to_json)
  list(topic = topic, nodes = nodes)
})

# Discover pkgdown URLs for linked packages from their DESCRIPTION
linked_packages <- unique(linked_packages)
urls <- structure(list(), names = character())
for (ext in linked_packages) {
  if (!requireNamespace(ext, quietly = TRUE)) next
  tryCatch({
    desc <- packageDescription(ext)
    if (!is.null(desc$URL)) {
      url_list <- trimws(unlist(strsplit(desc$URL, "[,[:space:]]+")))
      url_list <- url_list[nzchar(url_list)]
      pkgdown_url <- grep(
        "github\\.io|r-lib\\.org|tidyverse\\.org|tidymodels\\.org|bioconductor\\.org",
        url_list, value = TRUE)[1]
      if (!is.na(pkgdown_url)) {
        urls[[ext]] <- sub("/$", "", pkgdown_url)
      }
    }
  }, error = function(e) NULL)
}

cat(jsonlite::toJSON(
  list(topics = result, urls = urls),
  auto_unbox = TRUE, pretty = FALSE
))
