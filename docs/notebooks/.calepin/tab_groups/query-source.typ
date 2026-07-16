#import "/.calepin/calepin.typ" as calepin

#set document(title: [Synchronized tab groups])

#title()

Give multiple tab containers the same `group` name to keep their selected panels synchronized in HTML output. The first two containers below belong to the `language` group. Selecting R or Python in either one changes both.

= First synchronized container

#calepin.elements.tabs(group: "language")[
  #calepin.elements.tab("R", active: true)[
    The first container is showing its R content.
  ]

  #calepin.elements.tab("Python")[
    The first container is showing its Python content.
  ]
]

= Second synchronized container

#calepin.elements.tabs(group: "language")[
  #calepin.elements.tab("R", active: true)[
    The second container follows the first container to R.
  ]

  #calepin.elements.tab("Python")[
    The second container follows the first container to Python.
  ]
]

= Independent container

This container has no `group` argument, so its selection changes independently of the two containers above.

#calepin.elements.tabs[
  #calepin.elements.tab("R")[
    This independent container is showing its R content.
  ]

  #calepin.elements.tab("Python", active: true)[
    This independent container is showing its Python content.
  ]
]
