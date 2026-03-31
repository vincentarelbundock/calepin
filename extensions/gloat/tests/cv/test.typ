#import "/src/lib.typ" as gloat

// cv should pass when called without arguments
#gloat.cv.with()

// cv should pass when shown with full contacts
#show: gloat.cv.with(
  author: "Jacky Cao",
  address: "Cleveland OH, US",
  contacts: (
    [#link("mailto:email@domain")[email\@domain]],
    [#link("your-website-url")[personal website]],
    [#link("https://github.com/user")[gh/user]],
    [#link("https://www.linkedin.com/in/user/")[in/user]],
  ),
)
