export default function registerDoubleValue(vega) {
  vega.expressionFunction("doubleValue", value => value * 2)
}
