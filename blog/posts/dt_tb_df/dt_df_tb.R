# TODO: Chiro feedbak #4,5 ; McDermott feedback #2

## ---- data.caption
## ---- filter01.caption
Filter rows using an intger vector. Positive indices keep the specified rows.

## ---- filter01.datatable
DT[3:4,]

## ---- filter01.base
DF[3:4,]

## ---- filter01.dplyr
TB |> slice(3:4)

## ---- filter02.caption
Filter rows using an intger vector. Negative indices keep the specified rows.

## ---- filter02.datatable
DT[!3:7,]

## ---- filter02.dplyr
TB |> slice(-(3:7))

## ---- filter02.base
DF[-(3:7),]

## ---- filter03.caption
Filter rows using a logical vector. Keep the rows where the condition is TRUE. `%chin%` is a fast version of `%in%`, optimized for strings.

## ---- filter03.datatable
DT[V2 > 5]
DT[V4 %chin% c("A", "C")]

## ---- filter03.dplyr
TB |> filter(V2 > 5)
TB |> filter(V4 %in% c("A", "C"))

## ---- filter03.base
subset(DF, V2 > 5)
subset(DF, V4 %in% c("A", "C"))

## ---- filter04.caption
Filter rows based on multiple conditions.

## ---- filter04.datatable
DT[V1 == 1 & V4 == "A"] 

## ---- filter04.dplyr
TB |> filter(V1 == 1, V4 == "A")

## ---- filter04.base
subset(DF, V1 == 1 & V4 == "A")

## ---- filter05.caption
Keep unique rows.

## ---- filter05.datatable
unique(DT)
unique(DT, by = c("V1", "V4"))

## ---- filter05.dplyr
TB |> distinct()
TB |> distinct(V1, V4, .keep_all = TRUE)

## ---- filter05.base
DF[!duplicated(DF), ]
DF[!duplicated(DF[c("V1", "V4")]), ]

## ---- filter06.caption
Drop rows with missing values in specified columns.

## ---- filter06.datatable
na.omit(DT, cols = 1:4)

## ---- filter06.dplyr
TB |> tidyr::drop_na(1:4)

## ---- filter06.base
DF[complete.cases(DF[, 1:4]), ]

## ---- filter07.caption
Draw a random sample of rows.

## ---- filter07.datatable
DT[sample(.N, 3)]
DT[sample(.N, .N / 2)]

## ---- filter07.dplyr
TB |> slice_sample(n = 3)
TB |> slice_sample(prop = 0.5)

## ---- filter07.base
DF[sample(nrow(DF), 3), ]
DF[sample(nrow(DF), nrow(DF) / 2), ]

## ---- filter08.caption
Other filtering operations.

## ---- filter08.datatable
DT[V4 %like% "^B"]
DT[V2 %between% c(3, 5)]
DT[data.table::between(V2, 3, 5, incbounds = FALSE)]
DT[V2 %inrange% list(-1:1, 1:3)]

## ---- filter08.dplyr
TB |> filter(grepl("^B", V4))
TB |> filter(dplyr::between(V2, 3, 5))
TB |> filter(V2 > 3 & V2 < 5)
TB |> filter(V2 >= -1:1 & V2 <= 1:3)

## ---- filter08.base
subset(DF, grepl("^B", V4))
subset(DF, V2 >= 3 & V2 <= 5)
subset(DF, V2 > 3 & V2 < 5)
subset(DF, V2 %in% c(-1:1, 1:3))

## ---- sort01.caption
Sort rows in ascending order.

## ---- sort01.datatable
DT[order(V3)]

## ---- sort01.dplyr
TB |> arrange(V3)

## ---- sort01.base
sort_by(DF, ~V3)

## ---- sort02.caption
Sort rows in decreasing order.

## ---- sort02.datatable
DT[order(-V3)]

## ---- sort02.dplyr
TB |> arrange(desc(V3))

## ---- sort02.base
sort_by(DF, ~list(-V3))

## ---- sort03.caption
Sort rows by multiple columns.

## ---- sort03.datatable
DT[order(V1, -V2)]

## ---- sort03.dplyr
TB |> arrange(V1, desc(V2))

## ---- sort03.base
sort_by(DF, ~list(V1, -V2))

## ---- sort04.caption
Order rows in ascending or descending order. Using the `setorder()` or `setorderv()` functions is the most memory-efficient approach in `data.table`, because it reorders data *in place*. The `setorder()` uses non-standard evaluation to sort by unquoted column names, whereas the `setorderv()` function accepts a vector of column names.

## ---- sort04.datatable
setorder(DT, V4, -V1)
setorderv(DT, c("V4", "V1"), c(1, -1))

## ---- sort04.dplyr
TB = TB |> arrange(V4, desc(V1))

## ---- sort04.base
DF = DF[order(DF$V4, -DF$V1), ]
DF

## ---- selectCols01.caption
Extract one column as a vector.

## ---- selectCols01.datatable
DT[[3]]
DT[["V3"]]
DT[, V3]

## ---- selectCols01.dplyr
TB[[3]]
TB[["V3"]]
TB |> pull(V3)

## ---- selectCols01.base
DF[[3]]
DF[["V3"]]
DF[, 3, drop = TRUE]

## ---- selectCols02.caption
Extract one column as a data frame.

## ---- selectCols02.datatable
DT[, "V3"]
DT[, .SD, .SDcols = "V3"]

## ---- selectCols02.dplyr
TB[, "V3"]
TB |> select(V3)

## ---- selectCols02.base
DF[, 3, drop = FALSE]
DF[, "V3", drop = FALSE]

## ---- selectCols03.caption
Select several columns by column names.

## ---- selectCols03.datatable
DT[, .(V2, V3, V4)]
DT[, V2:V4]
DT[, .SD, .SDcols = V2:V4]
DT[, .SD, .SDcols = c("V2", "V3", "V4")]
cols = c("V2", "V3")
DT[, ..cols]

## ---- selectCols03.dplyr
TB |> select(V2, V3, V4)
TB |> select(V2:V4)
TB |> select(any_of(c("V2", "V3", "V4")))
cols = c("V2", "V3")
DF |> select(!!cols)

## ---- selectCols03.base
DF[, c("V2", "V3", "V4")]
subset(DF, select = c("V2", "V3", "V4"))
cols = c("V2", "V3")
DF[, cols]
DF[ , names(DF) %in% cols]

## ---- selectCols04.caption
Exclude several columns by column name.

## ---- selectCols04.datatable
DT[, !c("V2", "V3")]
DT[, .SD, .SDcols = !c("V2", "V3")]

## ---- selectCols04.dplyr
TB |> select(-V2, -V3)

## ---- selectCols04.base
DF[ , !(names(DF) %in% c("V2", "V3"))]

## ---- selectCols07.caption
Remove a column from the data set. Using `let()` is efficient because it modifies the data set in place.

## ---- selectCols07.datatable
DT[, let(V5 = NULL)]

## ---- selectCols07.dplyr
TB = TB |> select(-V5)

## ---- selectCols07.base
DF = DF[, !names(DF) %in% "V5"]

## ---- selectCols08.caption
Remove several columns from the data set. Using `:=` is efficient because it modifies the data set in place.

## ---- selectCols08.datatable
cols = c("V6", "V7")
DT[, (cols) := NULL]

## ---- selectCols08.dplyr
TB = TB |> select(-V6, -V7)

## ---- selectCols08.base
DF = DF[, !(names(DF) %in% c("V6", "V7"))]

## ---- selectCols06.caption
Complex selections using regular expressions or dedicated functions.

## ---- selectCols06.datatable
DT[, .SD, .SDcols = c("V1", "V2")]
DT[, .SD, .SDcols = patterns("^V[1-2]$")]
DT[, .SD, .SDcols = patterns("V")]
DT[, .SD, .SDcols = patterns("3$")]
DT[, .SD, .SDcols = patterns(".2")]
DT[, .SD, .SDcols = patterns("^V1$|^X$")]
DT[, .SD, .SDcols = patterns("^(?!V2)", perl = TRUE)]

## ---- selectCols06.dplyr
TB |> select(V1, V2)
TB |> select(num_range("V", 1:2))
TB |> select(contains("V"))
TB |> select(ends_with("3"))
TB |> select(matches(".2"))
TB |> select(one_of(c("V1", "X")))
TB |> select(-starts_with("V2"))

## ---- selectCols06.base
DF[, c("V1", "V2")]
DF[ , grep("^V[1-2]$", names(DF))]
DF[ , c("V4", setdiff(names(DF), "V4"))]
DF[ , grep("V", names(DF))]
DF[ , grep("3$", names(DF))]
DF[ , grep(".2", names(DF))]
DF[ , c("V1", "X")]
DF[ , !grepl("^V2", names(DF))]

## ---- rename01.caption
Select and rename.

## ---- rename01.datatable
DT[, .(X1 = V1, X2 = V2)]

## ---- rename01.dplyr
TB |> select(X1 = V1, X2 = V2)

## ---- rename01.base
setNames(
  DF[, c("V1", "V2")],
  c("X1", "X2"))

## ---- rename02.caption
Using the `data.table::setnames()` to rename columns is efficient because it renames column in place.

## ---- rename02.datatable
DT2 = copy(DT) # copy to avoid renaming the original data in place
setnames(DT2, old = c("V1", "V2"), new = c("X1", "X2"))

## ---- rename02.dplyr
TB2 = TB |> rename(X1 = V1, X2 = V2)

## ---- rename02.base
DF2 = DF
colnames(DF2)[match(c("V1", "V2"), colnames(DF2))] = c("X1", "X2")

## ---- summarise01.caption
Create a new data frame with a single row and a single column, summarizing the information of one column. Named or unnamed results.

## ---- summarise01.datatable
DT[, sum(V1)]
DT[, .(sumV1 = sum(V1))]

## ---- summarise01.dplyr
TB |> summarise(sum(V1))
TB |> summarise(sumV1 = sum(V1))

## ---- summarise01.base
sum(DF$V1)
data.frame(sumV1 = sum(DF$V1))

## ---- summarise02.caption
Create a new data frame with a single row and two columns, summarizing the information of one column. Named or unnamed results.

## ---- summarise02.datatable
DT[, .(sum(V1), sd(V3))]

## ---- summarise02.dplyr
TB |> summarise(sum(V1), sd(V3))

## ---- summarise02.base
data.frame(sum(DF$V1), sd(DF$V3))

## ---- summarise03.caption
Create a new data set with a single row and several columns, each with a new name corresponding to the summarized column in the initial data set.

## ---- summarise03.datatable
DT[, .(
  sumv1 = sum(V1),
  sdv3  = sd(V3))]

## ---- summarise03.dplyr
TB |>
  summarise(
   sumv1 = sum(V1),
   sdv3  = sd(V3))

## ---- summarise03.base
data.frame(
  sumv1 = sum(DF$V1),
  sdv3  = sum(DF$V3))

## ---- summarise05.caption
More summaries.

## ---- summarise05.datatable
DT[, data.table::first(V3)]
DT[, data.table::last(V3)]
DT[5, V3]
DT[, uniqueN(V4)]
uniqueN(DT)

## ---- summarise05.dplyr
TB |> summarise(dplyr::first(V3))
TB |> summarise(dplyr::last(V3))
TB |> summarise(nth(V3, 5))
TB |> summarise(n_distinct(V4))
n_distinct(TB)

## ---- summarise05.base
DF[1, "V3"]
DF[nrow(DF), "V3"]
DF[5, "V3"]
length(unique(DF$V4))
nrow(unique(DF))

## ---- cols01.caption
Modify a column.

## ---- cols01.datatable
DT[, let(V1 = V1^2)]
DT

## ---- cols01.dplyr
TB = TB |> mutate(V1 = V1^2)

## ---- cols01.base
DF$V1 = DF$V1^2

## ---- cols02.caption
Create a new column.

## ---- cols02.datatable
DT[, let(V5 = log(V1))]

## ---- cols02.dplyr
TB = mutate(DF, V5 = log(V1))

## ---- cols02.base
DF$V5 = log(DF$V1)

## ---- cols03.caption
Create several new columns.

## ---- cols03.datatable
DT[, let(
  V6 = sqrt(V1),
  V7 = "X")]

## ---- cols03.dplyr
TB = TB |> mutate(
  V6 = sqrt(V1),
  V7 = "X")

## ---- cols03.base
DF$V6 = sqrt(DF$V1)
DF$V7 = "X"

## ---- cols08.caption
Replace values in rows that match a condition applied to a column.

## ---- cols08.datatable
DT[V2 < 4, let(V2 = 0)]

## ---- cols08.dplyr
TB = DF |> 
  mutate(V2 = base::replace(V2, V2 < 4, 0))

## ---- cols08.base
DF$V2 = replace(DF$V2, DF$V2 < 4, 0)

## ---- by01.caption
Group summaries of a column in categories.

## ---- by01.datatable
DT[, by = "V4", .(sumV2 = sum(V2))]

## ---- by01.dplyr
TB |>
  group_by(V4) |>
  summarise(sumV2 = sum(V2))

## ---- by01.base
aggregate(V2 ~ V4, data = DF, FUN = sum)

## ---- by03.caption
Group values of a column in groups while applying a function to each category.

## ---- by03.datatable
DT[,
  by = tolower(V4),
  .(sumV1 = sum(V1))]

## ---- by03.dplyr
TB |>
  group_by(tolower(V4)) |>
  summarise(sumV1 = sum(V1))

## ---- by03.base
aggregate(
  V1 ~ tolower(V4),
  data = DF,
  FUN = sum)

## ---- by05.caption
Group values of a column in two categories, TRUE (for rows matching the condition) and FALSE (For rows not matching the condition).

## ---- by05.datatable
DT[,
  keyby = V4 == "A",
  sum(V1)]

## ---- by05.dplyr
TB |>
  group_by(V4 == "A") |>
  summarise(sum(V1))

## ---- by05.base
aggregate(V1 ~ groupA,
  data = transform(DF, groupA = V4 == "A"),
  FUN = sum)

## ---- by06.caption
Group values of a column in several categories with some of the rows of the initial dataset removed.

## ---- by06.datatable
DT[1:5,
  by = V4,
  .(sumV1 = sum(V1))]

## ---- by06.dplyr
TB |>
  slice(1:5) |>
  group_by(V4) |>
  summarise(sumV1 = sum(V1))

## ---- by06.base
aggregate(V1 ~ V4,
  data = DF[1:5,],
  FUN = sum)

## ---- by07.caption
Count the number of observation by group.

## ---- by07.datatable
DT[, .N, by = V4]

## ---- by07.dplyr
TB |>
  group_by(V4) |>
  tally()

## ---- by07.base
as.data.frame(table(DF$V4))

## ---- by08.caption
Add a new column with the number of observations per group.

## ---- by08.datatable
DT[, let(n = .N), by = V1]

## ---- by08.dplyr
TB = TB |>
  group_by(V1) |>
  add_tally()

## ---- by08.base
DF$n <-
  ave(DF$V1,
  DF$V1,
  FUN = length)

## ---- by09.caption
Retrieve the first/last/nth observation for each group.

## ---- by09.datatable
DT[, data.table::first(V2), by = V4]
DT[, data.table::last(V2), by = V4]
DT[, V2[2], by = V4]

## ---- by09.dplyr
TB |>
  group_by(V4) |>
  summarise(dplyr::first(V2))
TB |>
  group_by(V4) |>
  summarise(dplyr::last(V2))
TB |>
  group_by(V4) |>
  summarise(dplyr::nth(V2, 2))

## ---- by09.base
aggregate(V2 ~ V4,
  data = DF,
  FUN = function(x) x[1])
aggregate(V2 ~ V4,
  data = DF,
  FUN = function(x) x[length(x)])
aggregate(V2 ~ V4,
  data = DF,
  FUN = function(x) x[2])

## ---- by10.caption
Add a group counter column. Returns the initial dataset added with a group counter column

## ---- by10.datatable
DT[, let(Grp = .GRP), by = .(V4, V1)]
DT[, let(Grp = NULL)] # drop the group id

## ---- by10.dplyr
TB |>
  group_by(V4, V1) |>
  mutate(Grp = cur_group_id())

## ---- by10.base
# TODO

## ---- advCols01.caption
Summarise all the columns, typically using an aggregation function.

## ---- advCols01.datatable
DT[, lapply(.SD, max)]

## ---- advCols01.dplyr
TB |> summarise(across(everything(), max))

## ---- advCols01.base
apply(DF, 2, max)

## ---- advCols02.caption
Summarise several columns, typically using an aggregation function.

## ---- advCols02.datatable
DT[, lapply(.SD, mean),
  .SDcols = c("V1", "V2")]

## ---- advCols02.dplyr
TB |> 
  summarise(across(c(V1, V2), mean))

## ---- advCols02.base
data.frame(
  mean_V1 = mean(DF$V1),
  mean_V2 = mean(DF$V2))

## ---- advCols03.caption
Summarise several columns by groups, typically using an aggregation function.

## ---- advCols03.datatable
DT[, by = V4,
  lapply(.SD, mean),
  .SDcols = c("V1", "V2")]
DT[, by = V4,
  lapply(.SD, mean),
  .SDcols = patterns("V1|V2|Z0")]

## ---- advCol03.dplyr
TB |>
  group_by(V4) |>
  summarise(across(c(V1, V2), mean)) |>
  ungroup()

## ---- advCols03.base
cols = intersect(c("V1", "V2", "Z0"), names(DF))
aggregate(DF[cols], by = list(DF$V4), FUN = mean, na.rm = TRUE)

## ---- advCols04.caption
Summarise several columns by group using multiple aggregation functions, grouping by one or more variables.

## ---- advCols04.datatable
DT[, by = V4,
  c(lapply(.SD, sum),
  lapply(.SD, mean))]

## ---- advCols04.dplyr
TB |>
  group_by(V4) |>
  summarise(across(everything(),
  list(sum = sum, mean = mean)))

## ---- advCols04.base
aggregate(cbind(DF$V1, DF$V2, DF$V3) ~ V4,
  data = DF,
  FUN = function(x) c(sum = sum(x), mean = mean(x)))

## ---- advCols05.caption
Summarise a subest of columns by column type or condition.

## ---- advCols05.datatable
DT[, lapply(.SD, mean), .SDcols = is.numeric]
foo = function(x) {is.numeric(x) && mean(x) > 3}
DT[, lapply(.SD, mean), .SDcols = foo]

## ---- advCols05.dplyr
TB |>
  summarise(across(where(is.numeric),
  mean))
TB |> summarise(across(
  where(~ is.numeric(.x) && mean(.x) > 3), mean))

## ---- advCols05.base
sapply(DF[sapply(DF, is.numeric)],
  mean)
sapply(DF[sapply(DF, \(x) {
   is.numeric(x) && mean(x) > 3
})], mean)

## ---- advCols06.caption
Modify all the columns using the same function.

## ---- advCols06.datatable
DT[, lapply(.SD, rev)]

## ---- advCols06.dplyr
TB |> mutate(across(everything(), rev))

## ---- advCols06.base
data.frame(lapply(DF, rev))

## ---- advCols07.caption
Apply a transformation to each element of the variables selected.

## ---- advCols07.datatable
DT[, lapply(.SD, sqrt), .SDcols = V1:V2]
DT[, lapply(.SD, exp), .SDcols = !"V4"]

## ---- advCol07.dplyr
TB |> transmute(across(c(V1, V2), sqrt))
TB |> transmute(across(-any_of("V4"), exp))

## ---- advCols07.base
DF[c("V1", "V2")] = lapply(DF[c("V1", "V2")], sqrt)
DF[setdiff(names(DF), "V4")] = lapply(DF[setdiff(names(DF), "V4")], exp)

## ---- advCols08.caption
Apply a transformation for each element of the variables selected.

## ---- advCols08.datatable
DT[, names(.SD) := lapply(.SD, sqrt),
  .SDcols = c("V1", "V2")]

## ---- advCols08.dplyr
TB = TB |>
  mutate(across(all_of(c("V1", "V2")), sqrt))

## ---- advCols08.base
DF[c("V1", "V2")] = lapply(DF[c("V1", "V2")], sqrt)

## ---- advCols09.caption
Apply a transformation to each element of the variables selected by condition.

## ---- advCols09.datatable
DT[, .SD - 1, .SDcols = is.numeric]

## ---- advCols09.dplyr
TB |>
  transmute(across(where(is.numeric),
  ~ '-'(., 1L)))

## ---- advCols09.base
data.frame(lapply(DF,
  \(x) if (is.numeric(x)) x - 1 else x))

## ---- advCols10.caption
Apply a transformation to each element of the variables selected by condition.

## ---- advCols10.datatable
DT[, names(.SD) := lapply(.SD, as.integer),
  .SDcols = is.numeric]

## ---- advCols10.dplyr
TB = TB |>
  mutate(across(where(is.numeric),
  as.integer))

## ---- advCols10.base
DF[sapply(DF, is.numeric)] <-
  lapply(DF[sapply(DF, is.numeric)],
  as.integer)

## ---- advCols11.caption
Combine multiple functions in a single statement.

## ---- advCols11.datatable
DT[, by = V4,
  .(V1[1:2], "X")]

## ---- advCols11.dplyr
TB |>
  group_by(V4) |>
  slice(1:2) |>
  transmute(V1 = V1,
  V2 = "X")

## ---- advCols11.base
DF = do.call(rbind,
  by(DF, DF$V4, function(sub) {
  head(data.frame(V1 = sub$V1,
  V2 = "X"), 2)
}))

## ---- advCols12.caption
Use one or multiple expressions (with DT[,{j}]), where j is an arbitrary expression. Note: expression in curly braces only return the last result

## ---- advCols12.datatable
DT[, {print(V1)
  print(summary(V1))
  x = V1 + sum(V2)
  .(A = 1:.N, B = x)
}]

## ---- advCols12.dplyr
#

## ---- advCols12.base
{print(DF$V1)
print(summary(DF$V1))
x = DF$V1 + sum(DF$V2)
data.frame(A = seq_along(DF$V1), B = x)}

## ---- chain01.caption
Expression chaining, either horizontally (using `DT[][]`), meaning successively performed operations on the output in one single statement, without the need of intermediate results

## ---- chain01.datatable
DT[, by = V4, .(V1sum = sum(V1)) ][
  V1sum > 5]

## ---- chain01.dplyr
TB |>
  group_by(V4) |>
  summarise(V1sum = sum(V1)) |>
  filter(V1sum > 5)

## ---- chain01.base
DF = subset(aggregate(V1 ~ V4,
  data = DF,
  FUN = sum), V1 > 5)

## ---- chain02.caption
Expression chaining using `|>`. As chain operation `DT[][]`, successively performed operations on the output in one single statement, without the need of intermediate results

## ---- chain02.datatable
DT[, by = V4, .(V1sum = sum(V1))][
  order(-V1sum)]

## ---- chain02.dplyr
TB |>
  group_by(V4) |>
  summarise(V1sum = sum(V1)) |>
  arrange(desc(V1sum))

## ---- chain02.base
subset(aggregate(V1 ~ V4,
  data = DF,
  FUN = sum),
  V1 > 5)

## ---- key01.caption
Sort dataset and the column passed in argument becomes key. Output is an object the same type as the object indexed. Note: in `data.table::setkey`, the columns are always sorted in ascending order. In other cases, columns can be sorted in ascending or descending order. Also, for all methods the input is modified by reference and returned invisibly

## ---- key01.datatable
setkey(DT, V4)
setindex(DT, V4)
DT

## ---- key01.dplyr
TB = TB |> arrange(V4)
TB

## ---- key01.base
DF[order(DF$V4), ]

## ---- key02.caption
Select the matching row. Output is an object the same type as the object indexed. Return a dataset with the row(s) matching the condition.

## ---- key02.datatable
DT["A", on = "V4"]
DT[c("A", "C"), on = .(V4)]

## ---- key02.dplyr
TB |> filter(V4 == "A")
TB |> filter(V4 %in% c("A", "C"))

## ---- key02.base
DF[DF$V4 == "A", ]
DF[DF$V4 %in% c("A", "C"), ]

## ---- key03.caption
Select the first matching row. Output is an object the same type as the object indexed. Return an object the same type as the input with the first row matching the condition. Note: In data.table indexing case, the default value for mult argument is all, which select all matching rows

## ---- key03.datatable
DT["B", on = "V4", mult = "first"]
DT[c("B", "C"), on = "V4", mult = "first"]

## ---- key03.dplyr
TB |>
  filter(V4 == "B") |>
  slice(1)
# ?

## ---- key03.base
DF[DF$V4 == "B", ][1, ]

## ---- key04.caption
Select the last matching row. Output is an object the same type as the object indexed. Return a dataset with the last row matching the condition. In data.table indexing case, the default value for mult argument is all

## ---- key04.datatable
DT["A", on = "V4", mult = "last"]

## ---- key04.dplyr
TB |>
  filter(V4 == "A") |>
  slice(n())

## ---- key04.base
DF[DF$V4 == "A", ][nrow(DF[DF$V4 == "A", ]), ]

## ---- key05.caption
Select both matching and unmatching rows, or just matching rows. Output is an object the same type as the object indexed. If both matching and unmatching rows are selected, return a dataset with all rows with NA values are produced for non-matching rows. You can choose whether to include non-matching rows with NA values or exclude them based on your query. Note: nomatch argument only accept NA and 0 as values

## ---- key05.datatable
DT[c("A", "D"), on = "V4", nomatch = NA]
DT[c("A", "D"), on = "V4", nomatch = 0]

## ---- key05.dplyr
TB |> filter(V4 %in% c("A", "D"))

## ---- key05.base
DF[DF$V4 %in% c("A", "D"), ]

## ---- key06.caption
Apply a function on all matching rows. Output type depends on the function applied. In this case, output is an integer

## ---- key06.datatable
DT[c("A", "C"), sum(V1), on = "V4"]

## ---- key06.dplyr
TB |>
  filter(V4 %in% c("A", "C")) |>
  summarise(sum(V1))

## ---- key06.base
sum(DF$V1[DF$V4 %in% c("A", "C")])

## ---- key07.caption
Modify values for matching row(s). Output the same type as the object indexed. Return a dataset with the modified values for matching row(s).

## ---- key07.datatable
DT["A", let(V1 = 0), on = "V4"]
DT

## ---- key07.dplyr
TB = TB |>
  mutate(V1 = base::replace(V1, V4 == "A", 0L)) |>
  arrange(V4)
TB

## ---- key07.base
DF$V1[DF$V4 == "A"] = 0

## ---- key08.caption
Use keys in by

## ---- key08.datatable
DT[!y, on = "V4", sum(V1), by = .EACHI]
DT[V4 != "B", by = V4, sum(V1)]

## ---- key08.dplyr
TB |>
  filter(V4 != "B") |>
  group_by(V4) |>
  summarise(sum(V1))

## ---- key08.base
aggregate(V1 ~ V4,
  data = DF[DF$V4 != "B", ],
  FUN = sum)

## ---- key09.caption
Set keys/indices for multiple columns. The columns passed in argument become keys. Return the dataset with the columns in input as keys. Output is the same type as the object in input

## ---- key09.datatable
setkey(DT, V4, V1)
setindex(DT, V4, V1)

## ---- key09.dplyr
TB = arrange(DF, V4, V1)

## ---- key09.base
DF[order(DF$V4, DF$V1), ]

## ---- key10.caption
Subset using multiple keys. Output is the same type as the initial object. Return the initial object with rows that are matching with keys

## ---- key10.datatable
DT[.("C", 1), on = .(V4, V1)]
DT[.(c("B", "C"), 1), on = .(V4, V1)]
DT[.(c("B", "C"), 1), on = .(V4, V1), which = TRUE]

## ---- key10.dplyr
TB |> filter(V1 == 1, V4 == "C")
TB |> filter(V1 == 1, V4 %in% c("B", "C"))

## ---- key10.base
DF[DF$V4 == "C" & DF$V1 == 1, ]
DF[DF$V4 %in% c("B", "C") & DF$V1 == 1, ]
which(DF$V4 %in% c("B", "C") & DF$V1 == 1)

## ---- key11.caption
Remove keys/indices. Output is the same type as the object in input. Return the object without any keys. Note: in data.table::setkey, the input is modified by reference and returned invisibly.

## ---- key11.datatable
setkey(DT, NULL)
setindex(DT, NULL)

## ---- key11.dplyr
TB

## ---- key11.base
DF

## ---- set04.caption
Reorders the columns of a dataset.

## ---- set04.datatable
setcolorder(DT, c("V4", "V1", "V2"))
DT

## ---- set04.dplyr
TB = TB |> select(V4, V1, V2)

## ---- set04.base
DF = DF[, c("V4", "V1", "V2")]
DF

## ---- advBy01.caption
Select first/last/nth row of each group. Output the same type as the object input. Return the initial dataset with only first/last/nth row of each group.

## ---- advBy01.datatable
DT[, .SD[1], by = V4]
DT[, .SD[c(1, .N)], by = V4]
DT[, tail(.SD, 2), by = V4]

## ---- advBy01.dplyr
TB |>
  group_by(V4) |> 
  slice(1)
TB |>
  group_by(V4) |>
  slice(1, n())
TB |>
  group_by(V4) |>
  group_map(~ tail(.x, 2))

## ---- advBy01.base
DF[ave(DF$V4,
  DF$V4,
  FUN = seq_along) == 1, ]
DF[ave(DF$V4,
  DF$V4,
  FUN = seq_along) %in% c(1, max(ave(DF$V4, DF$V4, FUN = seq_along))), ]
do.call(rbind, by(DF, DF$V4, function(x) tail(x, 2)))

## ---- advBy02.caption
Select first/last/nth row of each group, with nested query. Returns a dataset the same type as the dataset in input with only first/last row for each group with query applied.

## ---- advBy02.datatable
DT[, .SD[which.min(V2)], by = V4]

## ---- advBy02.dplyr
TB |>
  group_by(V4) |>
  arrange(V2) |>
  slice(1)

## ---- advBy02.base
DF[order(DF$V2), ][
  ave(DF$V4, DF$V4, FUN = seq_along) == 1,]

## ---- advBy04.caption
Get the row number of first (and last) observation by group.

## ---- advBy04.datatable
DT[, .I, by = V4]
DT[, .I[1], by = V4]
DT[, .I[c(1, .N)], by = V4]

## ---- advBy04.dplyr
TB |>
  group_by(V4) |>
  mutate(cur_group_rows()) |>
  ungroup()
TB |>
  group_by(V4) |>
  summarize(cur_group_rows()[1])
TB |>
  group_by(V4) |>
  reframe(cur_group_rows()[c(1, n())])

## ---- advBy04.base
sapply(split(DT, DT$V4), function(group) rownames(group))
sapply(split(DT, DT$V4), function(group) rownames(group)[1])
sapply(split(DT, DT$V4), function(group) c(rownames(group)[1],
  rownames(group)[nrow(group)]))

## ---- advBy05.caption
List-columns are columns where each element is a vector, data frame, or other object. In the first command, we create a list column where each group of `V4` has a vector of `V1` values. In the second command, we create a list-column where each group of `V4` has a data frame of all columns of the original data set except `V4`.

## ---- advBy05.datatable
DT[, .(.(V1)),  by = V4]
DT[, .(.(.SD)), by = V4]

## ---- advBy05.dplyr
TB |>
  group_by(V4) |>
  summarise(list(V1))
TB |>
  group_by(V4) |>
  group_nest()

## ---- advBy05.base
tapply(DF$V1, DF$V4,
  function(x) list(x))
split(DF, DF$V4)

## ---- readwrite01.caption
Write a dataset to a csv file.

## ---- readwrite01.datatable
fwrite(DT, "DT.csv")

## ---- readwrite01.dplyr
TB |> readr::write_csv("TB.csv")

## ---- readwrite01.base
write.csv(DF, "DF.csv", row.names = FALSE)

## ---- readwrite02.caption
Write a dataset to a tab-delimited file.

## ---- readwrite02.datatable
fwrite(DT, "DT.txt", sep = "\t")

## ---- readwrite02.dplyr
TB |> readr::write_delim("TB.txt", delim = "\t")

## ---- readwrite02.base
write.table(DF, "DF.txt", sep = "\t", row.names = FALSE, col.names = TRUE)

## ---- readwrite04.caption
Import a csv or tab-delimited file as a data frame (or data.table) format.

## ---- readwrite04.datatable
fread("DT.csv")
fread("DT.txt", sep = "\t")

## ---- readwrite04.dplyr
readr::read_csv("TB.csv")
readr::read_delim("TB.txt", delim = "\t")

## ---- readwrite04.base
read.csv("DF.csv")
read.table("DF.txt", sep = "\t", header = TRUE)

## ---- readwrite05.caption
Import selected (or non-discarded) columns from a csv file as data frame (or data.table) format.

## ---- readwrite05.datatable
fread("DT.csv", select = c("V1", "V4"))
fread("DT.csv", drop = "V4")

## ---- readwrite05.dplyr

## ---- readwrite05.base
read.csv("DF.csv")[, c("V1", "V4")]
read.csv("DF.csv")[, !(names(read.csv("DF.csv")) %in% "V4")]

## ---- readwrite06.caption
Import several csv files with rows of each bind together into one data frame (or data.table) format.

## ---- readwrite06.datatable
rbindlist(lapply(c("DT.csv", "DT.csv"), fread))

## ---- readwrite06.dplyr
c("TB.csv", "TB.csv") |>
  purrr::map_dfr(readr::read_csv)

## ---- readwrite06.base
do.call(rbind, lapply(c("DF.csv", "DF.csv"), read.csv))

## ---- readwrite7
file.remove(c("DT.csv", "TB.csv", "DF.csv",
  "DT.txt", "TB.txt", "DF.txt", "DT2.csv"))

## ---- reshape01.caption
Reshape data format from wide to long format.

## ---- reshape01.datatable
melt(DT,
  id.vars       = "V4",
  variable.name = "Variable",
  value.name    = "Value")

## ---- reshape01.dplyr
TB |> tidyr::pivot_longer(
  cols = c("V1", "V2", "V3"),
  names_to = "Variable",
  values_to = "Value")

## ---- reshape01.base
reshape(DF,
  varying = setdiff(names(DF), "V4"),
  v.names = "value",
  timevar = "variable",
  times = setdiff(names(DF), "V4"),
  direction = "long") |> dim()

## ---- reshape02.caption
Cast data (from long to wide).

## ---- reshape02.datatable
long = CJ(a = 1:2, b = 1:2, c = c("x", "y"))
long[, let(d = rnorm(8))]
dcast(long, a + b ~ c)

## ---- reshape02.dplyr
long = tidyr::expand_grid(a = 1:2, b = 1:2, c = c("x", "y"))
long$d = rnorm(8)
tidyr::pivot_wider(long,
  id_cols = c("a", "b"),
  names_from = "c",
  values_from = "d")

## ---- reshape02.base
long = expand.grid(a = 1:2, b = 1:2, c = c("x", "y"))
long$d = rnorm(8)
reshape(long, 
  idvar = c("a", "b"),
  timevar = "c",
  direction = "wide")

## ---- reshape03.caption
Separate data into groups based on a factor and return a list of data frames.

## ---- reshape03.datatable
split(DT, by = "V4")

## ---- reshape03.dplyr
TB |> group_split(V4)

## ---- reshape03.base
split(DF, DF$V4)

## ---- reshape04.caption
Split a single column into multiple columns based on a delimiter.

## ---- reshape04.datatable
tmp = data.table(a = c("A:a", "B:b", "C:c"))
tmp[, c("w", "z") := tstrsplit(a, split = ":")]

## ---- reshape04.dplyr
tmp = tibble(a = c("A:a", "B:b", "C:c"))
tidyr::separate(tmp, a, c("w", "z"), remove = FALSE)

## ---- reshape04.base
# TODO

## ---- other06.caption
Fast version of `ifelse()`. Return a vector the same length as the vector tested, with specific value for in case where the condition is `TRUE`, where it is `FALSE`, and where it is `NA`.

## ---- other06.datatable
x = c(-3:3, NA)
fifelse(test = x < 0,
  yes  = "neg",
  no   = "pos",
  na   = "NA")

## ---- other06.dplyr
x = c(-3:3, NA)
if_else(condition = x < 0,
  true      = "neg",
  false     = "pos",
  missing   = "NA")

## ---- other06.base
x = c(-3:3, NA)
result = ifelse(is.na(x),
  "NA",
  ifelse(x < 0, "neg", "pos"))
result

## ---- other07.caption
Recode several cases at once, based on a vector of conditions.

## ---- other07.datatable
x = 1:10
fcase(
  x %% 6 == 0, "fizz buzz",
  x %% 2 == 0, "fizz",
  x %% 3 == 0, "buzz",
  default = NA_character_
)

## ---- other07.dplyr
x = 1:10
case_when(
  x %% 6 == 0 ~ "fizz buzz",
  x %% 2 == 0 ~ "fizz",
  x %% 3 == 0 ~ "buzz",
  TRUE ~ as.character(x)
)

## ---- other07.base
x = 1:10
result = ifelse(x %% 6 == 0, "fizz buzz",
  ifelse(x %% 2 == 0, "fizz",
  ifelse(x %% 3 == 0, "buzz", as.character(x))))
result

## ---- join01.caption
Create example datasets for joins.

## ---- join01.datatable
x = data.table(Id  = c("A", "B", "C", "C"),
  X1  = c(1L, 3L, 5L, 7L),
  XY  = c("x2", "x4", "x6", "x8"),
  key = "Id")
y = data.table(Id  = c("A", "B", "B", "D"),
  Y1  = c(1L, 3L, 5L, 7L),
  XY  = c("y1", "y3", "y5", "y7"),
  key = "Id")

## ---- join01.base
x = data.frame(Id  = c("A", "B", "C", "C"),
  X1  = c(1L, 3L, 5L, 7L),
  XY  = c("x2", "x4", "x6", "x8"),
  key = "Id")
y = data.frame(Id  = c("A", "B", "B", "D"),
  Y1  = c(1L, 3L, 5L, 7L),
  XY  = c("y1", "y3", "y5", "y7"),
  key = "Id")

## ---- join01.dplyr
x = tibble(Id  = c("A", "B", "C", "C"),
  X1  = c(1L, 3L, 5L, 7L),
  XY  = c("x2", "x4", "x6", "x8"),
  key = "Id")
y = tibble(Id  = c("A", "B", "B", "D"),
  Y1  = c(1L, 3L, 5L, 7L),
  XY  = c("y1", "y3", "y5", "y7"),
  key = "Id")

## ---- join02.caption
Left join. Keep rows of `x`.

## ---- join02.datatable
merge(x, y, all.x = TRUE, by = "Id")
y[x, on = "Id"]

## ---- join02.dplyr
left_join(x, y, by = "Id")

## ---- join02.base
merge(x, y, by = "Id", all.x = TRUE)

## ---- join03.caption
Right join. Keep rows of `y`.

## ---- join03.datatable
merge(x, y, all.y = TRUE, by = "Id")
x[y, on = "Id"]

## ---- join03.dplyr
right_join(x, y, by = "Id")

## ---- join03.base
merge(x, y, by = "Id", all.y = TRUE)

## ---- join04.caption
Inner join. Keep rows shared by `x` and `y`.

## ---- join04.datatable
merge(x, y, by = "Id")
x[y, on = "Id", nomatch = NULL]

## ---- join04.dplyr
inner_join(x, y, by = "Id")

## ---- join04.base
merge(x, y, by = "Id")

## ---- join05.caption
Full join. Keep both the rows of `x` and `y`.

## ---- join05.datatable
merge(x, y, by = "Id", all = TRUE)

## ---- join05.dplyr
full_join(x, y, by = "Id")

## ---- join05.base
merge(x, y, by = "Id", all = TRUE)

## ---- join07.caption
Anti join. Keep rows from `x` with no match in `y`.

## ---- join07.datatable
x[!y, on = "Id"]

## ---- join07.dplyr
anti_join(x, y, by = "Id")

## ---- join07.base
# TODO

## ---- morejoins01.caption
Select columns while joining. When there are clashing column names in `data.table`, the `x.` prefix refers to columns in the data table before square brackets, and the `i.` prefix refers to columns in the index data table.

## ---- morejoins01.datatable
x[y, on = "Id", .(Id, X1, x.XY, i.XY)]

## ---- morejoins01.dplyr
right_join(x, y, by = "Id") |>
  select(Id, X1, XY.x, XY.y)

## ---- morejoins01.base
merge(x, y, all.y = TRUE) 
  subset(select = c("Id", "X1", "XY.x", "XY.y"))

## ---- morejoins02.caption
Summarize columns while joining.

## ---- morejoins02.datatable
y[x, .(X1Y1 = sum(Y1) * X1), by = .EACHI]

## ---- morejoins02.dplyr
y |>
  group_by(Id) |>
  summarise(SumY1 = sum(Y1)) |>
  right_join(x) |>
  mutate(X1Y1 = SumY1 * X1) |>
  select(Id, X1Y1)

## ---- morejoins02.base
merge(aggregate(Y1 ~ Id, data = y, FUN = sum),
  x, by = "Id", all.x = FALSE, all.y = TRUE) |>
  transform(X1Y1 = SumY1 * X1)[, c("Id", "X1Y1")]

## ---- morejoins03.caption
Update columns while joining.

## ---- morejoins03.datatable
y[x, let(SqX1 = i.X1^2)]
y[, let(SqX1 = x[.BY, X1^2, on = "Id"]), by = Id]
y[, let(SqX1 = NULL)]

## ---- morejoins03.dplyr
x |>
  select(Id, X1) |>
  mutate(SqX1 = X1^2) |>
  right_join(y, by = "Id") |>
  select(names(y), SqX1)

## ---- morejoins03.base
data.frame(merge(
  transform(x[, c("Id", "X1")], SqX1 = X1^2),
  y, by = "Id", all.x = FALSE, all.y = TRUE))[, c(names(y), "SqX1")]

## ---- morejoins04.caption
Adds a list column with rows from y matching x (nest-join). Leaves x (initial data set) completely unchanged and add a new list-column, where each element contains the rows from y that match the corresponding row in x.

## ---- morejoins04.datatable
x[, y := .(.(y[.BY, on = "Id"])), by = Id]
x[, let(y = NULL)]

## ---- morejoins04.dplyr
nest_join(x, y, by = "Id")

## ---- morejoins04.base
merge(x, y, by = "Id", all = TRUE) |> 
  transform(Nested = split(., .$Id))

## ---- morejoins05.caption
Update columns while joining, using vectors of column names.

## ---- morejoins05.datatable
cols  = c("NewXY", "NewX1")
icols = paste0("i.", c("XY", "X1"))
y[x, (cols) := mget(icols)]
y[, (cols) := NULL]

## ---- morejoins05.dplyr
# ?

## ---- morejoins05.base
cols = c("NewXY", "NewX1")
icols = paste0("i.", c("XY", "X1"))
y[, cols] = mget(icols)
y[, cols] = NULL

## ---- morejoins06.caption
Join passing columns to match in the 'on' argument

## ---- morejoins06.datatable
x[z, on = .(Id == ID, X1 == Z1)]

## ---- morejoins06.dplyr
right_join(x, z, by = c("X1" = "Z1"))

## ---- morejoins06.base
merge(x, z, by.x = "X1", by.y = "Z1", all.y = TRUE)

## ---- morejoins07.caption
Non-equi joins (perform a join using operators different from '='). These joins are useful for matching rows where values fall within a range or satisfy other inequalities.

## ---- morejoins07.datatable
x[z, on = .(Id == ID, X1 <= Z1)]
x[z, on = .(Id == ID, X1 > Z1)]
x[z, on = .(X1 < Z1), allow.cartesian = TRUE]

## ---- morejoins07.dplyr
#

## ---- morejoins07.base
subset(merge(x, z, by.x = "Id", by.y = "ID", all.x = TRUE), X1 <= Z1)
subset(merge(x, z, by.x = "Id", by.y = "ID", all.x = TRUE), X1 > Z1)
merge(x, z, by.x = "Id", by.y = "ID", all.x = TRUE)[X1 < Z1, ]

## ---- morejoins08.caption
Rolling joins/subsets (performed on the last numeric column)

## ---- morejoins08.datatable
x[z, on = .(Id == ID, X1 == Z1), roll = "nearest"]
setkey(x, Id, X1)
x[.("C", 5:9), roll = "nearest"]

## ---- morejoins08.dplyr
#

## ---- morejoins08.base
merge(x, z, by.x = c("Id", "X1"), by.y = c("ID", "Z1"), all.x = TRUE)
x[order(x$Id, x$X1)]
x[.("C", 5:9), roll = "nearest"]

## ---- morejoins09.caption
Join datasets

## ---- morejoins09.datatable
x[.("C", 5:9), roll = Inf]
x[.("C", 5:9), roll = 0.5]
x[.("C", 5:9), roll = Inf, rollends = c(FALSE, TRUE)]
x[.("C", 5:9), roll = Inf, rollends = c(FALSE, FALSE)]

## ---- morejoins09.dplyr
#

## ---- morejoins09.base
x[x$V1 == "C" & x$V2 %in% 5:9, ]
#x[subset(x, Id == "C", X1 %in% 5:9), ]
x[x$V1 == "C" & x$V2 >= 5 & x$V2 <= 9, ]
#x[subset(x, Id == "C", X1 >= 5 & X1 <= 9), ]
x[x$V1 == "C" & x$V2 > 5 & x$V2 < 9, ]
#jesaispas

## ---- morejoins10.caption
Rolling joins

## ---- morejoins10.datatable
x[.("C", 5:9), roll = -Inf]
x[.("C", 5:9), roll = -0.5]
x[.("C", 5:9), roll = -Inf, rollends = c(TRUE, FALSE)]
x[.("C", 5:9), roll = -Inf, rollends = c(TRUE, TRUE)]

## ---- morejoins10.dplyr
#

## ---- morejoins10.base
x[x$V1 == "C" & x$V2 <= 9 & x$V2 >= 5, ]
x[x$V1 == "C" & x$V2 >= 5 & x$V2 <= 9, ]
x[x$V1 == "C" & x$V2 > 5 & x$V2 < 9, ]
x[x$V1 == "C" & x$V2 >= 5 & x$V2 <= 9, ]

## ---- morejoins11.caption
Cross join ('CJ' ~ 'expand.grid'. Return a dataset is formed from the cross product of the vectors, in other words one row for each combination of vector in argument.

## ---- morejoins11.datatable
CJ(c(2, 1, 1), 3:2)
CJ(c(2, 1, 1), 3:2, sorted = FALSE, unique = TRUE)

## ---- morejoins11.dplyr
expand.grid(c(2, 1, 1), 3:2)
#

## ---- morejoins11.base
expand.grid(c(2, 1, 1), 3:2)
unique(expand.grid(c(2, 1, 1), 3:2))

## ---- bind01
x = data.table(1:3)
y = data.table(4:6)
z = data.table(7:9, 0L)

## ---- bind02.caption
Take datasets as argument and combine them by rows. Output is the same type as objects in argument if they are the same type. Return a dataset with rows bind together.

## ---- bind02.datatable
rbind(x, y)
rbind(x, z, fill = TRUE)

## ---- bind02.dplyr
bind_rows(x, y)
bind_rows(x, z)

## ---- bind02.base
rbind(x, y)
rbind(x, z, fill = TRUE)

## ---- bind03.caption
Bind rows using a list. Take a list of datasets as argument and combine them by rows. Return a dataset with the rows bound together.

## ---- bind03.datatable
rbindlist(list(x, y), idcol = TRUE)

## ---- bind03.dplyr
bind_rows(list(x, y), .id = "id")

## ---- bind03.base
rbind(cbind(x, id = 1), cbind(y, id = 2))

## ---- bind04.caption
Take datasets as argument and combine them by columns. Output is the same type as objects in argument if they are the same type. Return a dataset with columns bind together.

## ---- bind04.datatable
base::cbind(x, y)

## ---- bind04.dplyr
bind_cols(x, y)

## ---- bind04.base
cbind(x, y)

## ---- setOps02.caption
Intersection of two object placed in argument. Output is the same type as the two arguments. Return a dataset with rows in both x and y

## ---- setOps02.datatable
fintersect(x, y)

## ---- setOps02.dplyr
dplyr::intersect(x, y)

## ---- setOps02.base
intersect(x, y)

## ---- setOps03.caption
Set difference. Take a pair of datasets as arguments. Output is an object the same type as the objects in argument. Return elements present in x but not in y.

## ---- setOps03.datatable
fsetdiff(x, y)
fsetdiff(x, y, all = TRUE)

## ---- setOps03.dplyr
dplyr::setdiff(x, y)

## ---- setOps03.base
setdiff(x, y)

## ---- setOps04.caption
Unite elements from two distinct dataset. Take a pair of datasets in argument. Output is the same type as the first dataset in argument. Note: dplyr::union_all keeps duplicate

## ---- setOps04.datatable
funion(x, y)
funion(x, y, all = TRUE)

## ---- setOps04.dplyr
dplyr::union(x, y)
union_all(x, y)

## ---- setOps04.base
union(x, y)
c(x, y)

## ---- setOps05.caption
Test two objects for being exactly equal (including data structures). If they are, TRUE is returned. Otherwise, FALSE is returned

## ---- setOps05.datatable
fsetequal(x, x[order(-V1),])
all.equal(x, x)

## ---- setOps05.dplyr
setequal(x, x[order(-V1),])
dplyr::all_equal(x, x)

## ---- setOps05.base
identical(x, x)
all.equal(x, x)

## ---- sessionInfo
sessionInfo()
