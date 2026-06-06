= CLI reference
<cli-reference>

== `calepin`
<calepin>

```text
Preprocess Typst documents with executable code chunks

Usage: calepin <COMMAND>

Commands:
  new      Create a new example Typst file
  compile  Preprocess, then invoke typst compile
  watch    Watch, preprocess, and delegate recompiles to typst watch
  stop     Stop a running calepin watch process
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --version  Print version
  -h, --help     Print help
```

== `calepin compile`
<calepin-compile>

```text
Preprocess, then invoke typst compile

Usage: calepin compile [OPTIONS] <INPUT> [OUTPUT] [TYPST_ARGS]...

Arguments:
  <INPUT>
          Input .typ file

  [OUTPUT]
          Output path passed to typst compile

  [TYPST_ARGS]...
          Arguments forwarded to typst compile after `--`

Options:
      --format <FORMAT>
          Output format passed to typst compile
          
          [possible values: pdf, png, svg, html]

      --template <TEMPLATE>
          Output template applied after compilation.

          Use `basic` or `pico` to apply a built-in HTML theme.

          Possible values:
          - basic: Render HTML using the built-in `basic` theme
          - pico:  Render HTML using the built-in `pico` theme

      --results <RESULTS>
          Override results JSON path

      --clean
          Remove generated results and figures before preprocessing

  -q, --quiet
          Quiet mode

      --timeout <TIMEOUT>
          Per-chunk timeout in seconds

  -h, --help
          Print help (see a summary with '-h')
```

== `calepin watch`
<calepin-watch>

```text
Watch, preprocess, and delegate recompiles to typst watch

Usage: calepin watch [OPTIONS] <INPUT> [OUTPUT] [TYPST_ARGS]...

Arguments:
  <INPUT>          Input .typ file
  [OUTPUT]         Output path passed to typst watch
  [TYPST_ARGS]...  Arguments forwarded to typst watch after `--`

Options:
      --format <FORMAT>    Output format passed to typst watch [possible values: pdf, png, svg, html]
      --results <RESULTS>  Override results JSON path
      --clean              Remove generated results and figures before preprocessing
  -q, --quiet              Quiet mode
      --timeout <TIMEOUT>  Per-chunk timeout in seconds
  -h, --help               Print help
```

== `calepin stop`
<calepin-stop>

```text
Stop a running calepin watch process

Usage: calepin stop [INPUT]

Arguments:
  [INPUT]  Input .typ file to stop the matching calepin watch. Omit this value to stop all active watches under the current project's `.calepin` directory

Options:
  -h, --help  Print help
```
