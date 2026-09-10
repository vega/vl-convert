import { scaleLinear } from "https://esm.sh/d3-scale@4"

export default function registerScale(vega) {
  const scale = scaleLinear().domain([0, 1]).range([0, 100])
  vega.expressionFunction("scaledPercent", value => scale(value))
}
