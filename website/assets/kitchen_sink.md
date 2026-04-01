## Markdown basics {#sec-basics}
This section tests standard CommonMark and GFM features.

### Inline formatting {#inline-formatting}
*Italic*, **bold**, ***bold italic***, ~~strikethrough~~, and `inline code`.

### Lists {#lists}
Ordered:

- First item


- Second item


- Third item



Unordered:

- Alpha


- Beta


- Gamma



### Blockquote {#blockquote}
> This is a blockquote with **bold** and *italic* text.


### Links and images {#links-and-images}
A [link to nowhere](https://example.com) and a reference-style [link](https://example.com).

[Placeholder (200x100)]

### Footnotes {#footnotes}
Here is a sentence with a footnote[^1].

### Horizontal rule {#horizontal-rule}
---

### Table {#sec-tables}
| Name | Score | Grade |
|---|
| Alice | 95 | A |
| Bob | 87 | B |
| Carol | 92 | A |

## Code chunks {#code-chunks}
### R chunks {#r-chunks}



``` r
x <- 1:10
mean(x)
```

```
> [1] 5.5
```

#### Echo fenced {#echo-fenced}



``` r
```{r, chunk-2}
#| echo: fenced
summary(mtcars$mpg)
```
```

```
>    Min. 1st Qu.  Median    Mean 3rd Qu.    Max. 
>   10.40   15.43   19.20   20.09   22.80   33.90
```

#### Figure from R {#sec-rfig}



``` r
plot(1:10, (1:10)^2, pch = 19, col = "steelblue",
     xlab = "x", ylab = "x squared")
```

![](kitchen_sink_files/fig-scatter-1.svg)

*Scatter plot of x vs x squared*

See @fig-scatter for the scatter plot.

#### Echo false {#echo-false}


#### Eval false {#eval-false}



``` r
stop("this would error if evaluated")
```

#### Include false (silent execution) {#include-false-silent-execution}


#### Results asis {#results-asis}



``` r
cat("**This is bold from asis output.**\n")
```

**This is bold from asis output.**

#### Warning and message control {#warning-and-message-control}



``` r
warning("suppressed warning")
message("suppressed message")
cat("Only this output appears.\n")
```

```
> Only this output appears.
```

### Python chunks {#python-chunks}



``` python
import math
print(f"Pi is approximately {math.pi:.4f}")
```

```
> Pi is approximately 3.1416
```


``` python
squares = [x**2 for x in range(1, 6)]
print(squares)
```

```
> [1, 4, 9, 16, 25]
```

### Shell chunks {#shell-chunks}



``` sh
echo "Hello from shell"
```

```
> Hello from shell
```

### Inline code {#inline-code}
The mean of 1 to 10 is 5.5. Pi is 3.14.
The secret value is 42.



> ## Math {#sec-math}
### Inline math {#inline-math}
The Pythagorean theorem states $a^2 + b^2 = c^2$.

A literal dollar sign costs $5.

### Display math {#display-math}
$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

### Labeled equation {#labeled-equation}
$${{ clp.content }}$$ {#{{ cfg.id }}}

### Another labeled equation {#another-labeled-equation}
$${{ clp.content }}$$ {#{{ cfg.id }}}

See @eq-einstein and @eq-binomial.



## Cross-references {#sec-crossref}
We can reference sections (@sec-basics), figures (@fig-scatter), tables (Table 1), theorems (Theorem 1), definitions (Definition 1), and code listings (Listing 1).

Bracketed form: [@fig-scatter].



> Suppress type: [-@eq-einstein]. Grouped: (@eq-einstein; @eq-binomial).



## Callouts {#callouts}



> **ℹ️ Note**
>
> ### A note {#a-note}
This is a **note** callout with a custom title.




> **💡 Tip**
>
> A tip callout with the default title.




> **⚠️ Warning**
>
> ### Watch out {#watch-out}
A warning callout.




> **❗ Important**
>
> An important callout with the default title.




> **🔥 Caution**
>
> ### Collapsible caution {#collapsible-caution}
This caution callout starts collapsed.



## Theorems {#sec-theorems}


**Theorem 1.** *### Fermat’s Last Theorem {#fermats-last-theorem}
For $n > 2$, there are no positive integers $a$, $b$, $c$ such that $a^n + b^n = c^n$.

*

*Proof.* The proof was completed by Andrew Wiles in 1995 and is too long to include here.

 □

**Definition 1.** ### Group {#group}
A group is a set $G$ with a binary operation satisfying closure, associativity, identity, and invertibility.



**Lemma 1.** *Every finite group has an element of order dividing the group order.

*

**Corollary 1.** *Every group of prime order is cyclic.

*

**Example 1.** The integers under addition form a group.



**Remark 1.** Groups are fundamental to abstract algebra.



By Theorem 1 and Definition 1, we see the connection. Also see Lemma 1, Corollary 1, Example 1, and Remark 1.

## Tabsets {#tabsets}


### Tab A {#tab-a}
Content of the first tab.

### Tab B {#tab-b}
Content of the second tab with some math: $e^{i\pi} + 1 = 0$.

### Tab C {#tab-c}



``` r
cat("Code in a tab\n")
```

```
> Code in a tab
```

## Layouts {#layouts}
### Two-column layout {#two-column-layout}


Left column content. This paragraph appears on the left side of a two-column layout.

Right column content. This paragraph appears on the right side.



### Custom grid layout {#custom-grid-layout}


[A (150x100)]

[B (150x100)]

[C (150x100)]



## Figure divs {#figure-divs}


[Figure (300x200)]



*A figure div with a caption.*

See Figure 1.

## Table divs {#table-divs}


: Number of known moons for selected planets.

| Planet | Moons |
|---|
| Earth | 1 |
| Mars | 2 |
| Jupiter | 95 |



See Table 1.

## Code listings {#code-listings}



``` r
square <- function(x) x^2
square(7)
```

```
> [1] 49
```

See Listing 1.

## Content visibility {#content-visibility}


> This paragraph is hidden in HTML output but visible elsewhere.



> This paragraph is visible because `show_extra` is true.



## Hidden div {#hidden-div}


The hidden div executed: computed but not shown.

## Raw blocks {#raw-blocks}


**This is raw Markdown.**

## Jinja features {#jinja-features}
### Context variables {#context-variables}
The document title is: Kitchen Sink.

The sample size is: 50.

The current format is: markdown.

### Conditionals {#conditionals}
Rendering to markdown.

### Loops {#loops}
Features of this document:

- R code chunks


- Python code chunks


- Cross-references



### Built-in spans {#built-in-spans}
Lorem ipsum dolor sit amet consectetur adipiscing elit. Elit sed do eiusmod tempor incididunt ut labore et dolore magna.


---

After the page break.

## Syntax highlighting {#syntax-highlighting}
Non-executable code blocks with syntax highlighting:




``` javascript
function greet(name) {
  return `Hello, ${name}!`;
}
```


``` python
def factorial(n: int) -> int:
    return 1 if n <= 1 else n * factorial(n - 1)
```