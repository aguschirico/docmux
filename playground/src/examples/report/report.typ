#set document(title: "Quarterly Latency Report")

= Quarterly Latency Report

This report covers *API latency* for the third quarter. All numbers are
measured at the load balancer, in milliseconds.

== Summary

Median latency fell by 18% after the cache rollout. The tail did not move
as much: the 99th percentile is still dominated by cold starts.

#figure(
  table(
    columns: 3,
    table.header([Endpoint], [p50], [p99]),
    [`/search`], [42], [310],
    [`/orders`], [27], [190],
    [`/login`], [65], [480],
  ),
  caption: [Latency by endpoint, Q3.],
) <tab:latency>

== Model

We model a request's latency as the sum of queueing and service time:

$ T = W_q + 1 / mu, quad W_q = rho / (mu - lambda) $ <eq:latency>

where $lambda$ is the arrival rate, $mu$ the service rate and
$rho = lambda / mu$ the utilization. @eq:latency explains why the tail
grows quickly once $rho$ approaches 1.

== Next steps

+ Pre-warm instances before traffic peaks.
+ Move `/login` behind the same cache as `/search`.
+ Re-measure in four weeks.
