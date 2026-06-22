#import "@preview/classicthesis:0.1.0": *

#set document(
  title: [A simple exmaple],
  author: "John Doe",
)

#calepin.setup(
  vars: (
    title: "a great title",
    subtitle: "With a catchy subtitle",
    author: "Vincent Arel-Bundock",
    date: "2026-06-22",
  ),
)

#show: classicthesis.with(
    abstract: [This #strong[fascinating] thesis explores...],
)


= Top level

#lorem(100)

== Subsection

#lorem(100)
