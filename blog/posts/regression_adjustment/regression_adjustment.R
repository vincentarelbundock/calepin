library('MASS')
library('estimatr')
library('data.table')
library('future.apply')
plan(multicore, workers = 5)

simulate = function(
  idx,
  N = 100,
  rho = 0.1,
  heterogeneity = "Strong") {

  # coefficient heterogeneity
  gamma = list(
    None = list(
      Y0 = c(2, 2, -2, -.05, -.02, .3),
      Y1 = c(3, 2, -2, -.05, -.02, .3)),
    Mild = list(
      Y0 = c(2, 2, -2, -.05, -.02, .3),
      Y1 = c(3, 1, -1, -.05, -.03, -.6)),
    Strong = list(
      Y0 = c(0, 1, -1, -.05, .02, .6),
      Y1 = c(1, -1, 1.5, .03, -.02, -.6)))

  g0 = gamma[[heterogeneity]]$Y0
  g1 = gamma[[heterogeneity]]$Y1

  # correlated covariates
  dat = data.table(mvrnorm(
    n = N, 
    mu = c(1, 2), 
    Sigma = matrix(c(2, .5, .5, 3), ncol = 2)))
  colnames(dat) = c("X1", "X2")

  # treatment 
  dat[, W := rbinom(N, 1, prob = rho)]

  # outcome
  Z = model.matrix(~X1 + X2 + I(X1^2) + I(X2^2) + X1:X2, dat)
  dat[, Y0 := Z %*% g0 + rnorm(N)][
      , Y1 := Z %*% g1 + rnorm(N)][
      , Y  := W * Y1 + (1 - W) * Y0]

  return(dat)
}


fit = function(dat) {
  out = data.table(
    sdim = dat[, mean(Y[W == 1]) - mean(Y[W == 0])],
    ra = coef(lm(Y ~ W + X1 + X2, dat))["W"],
    lin = coef(lm_lin(Y ~ W, ~X1 + X2, dat))["W"]
  )
  return(out)
}


montecarlo = function(
  nsims = 1000,
  N = 100,
  rho = 0.1,
  heterogeneity = "Strong") {

  results = list()

  for (i in 1:nsims) {
    dat = simulate(
      idx = idx, 
      N = N,
      rho = rho, 
      heterogeneity = heterogeneity)

    out = fit(dat)

    out$idx = i
    out$N = N
    out$rho = rho
    out$heterogeneity = heterogeneity

    results[[i]] = out
  }

  results = rbindlist(results)

  return(results)
}

params = CJ(
  N = c(100, 500, 1000),
  rho = seq(.1, .9, length.out = 20),
  heterogeneity = c("Strong", "Mild", "None"))

results = future_Map(
  montecarlo,
  nsims = 1000,
  N = params$N,
  rho = params$rho,
  heterogeneity = params$heterogeneity,
  future.seed = TRUE)

results = rbindlist(results)

results = melt(results,
  id.vars = c("idx", "N", "rho", "heterogeneity"),
  variable.name = "model")

results = results[!value %in% c(-Inf, Inf), .(
  rmse = sqrt(mean((value - 1)^2)),
  bias = mean(value - 1),
  sd = sd(value)),
  by = .(model, N, rho, heterogeneity)]

results[
  , rmse_relative := rmse / min(rmse),
  by = .(N, rho, heterogeneity)]

fwrite(results, file = "lin.csv")
