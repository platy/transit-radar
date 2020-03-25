import { SVG } from './node_modules/@svgdotjs/svg.js/dist/svg.esm.js'
var draw = SVG().addTo('body').size(1100, 1400)
const xmax = 1000, ymax = 1000
const maxSeconds = 30 * 60
draw.circle((10 * 60 / maxSeconds) * xmax).attr({ cx: xmax / 2, cy: ymax / 2, class: 'grid'})
draw.circle((20 * 60 / maxSeconds) * xmax).attr({ cx: xmax / 2, cy: ymax / 2, class: 'grid'})
draw.circle((30 * 60 / maxSeconds) * xmax).attr({ cx: xmax / 2, cy: ymax / 2, class: 'grid'})

function drawStop(stop) {
  let {
    name,
  } = stop
  let [cx, cy] = stopCoords(stop)
  const stopDia = 6
  draw.circle(stopDia).attr({ cx, cy })
  draw.text(name).move(cx + stopDia + 2, cy - 6)
  draw.text
}

function stopCoords({
  bearing,
  seconds,
}, secondsOverride) {
  let h = (secondsOverride || seconds) / maxSeconds
  let [x, y] = [h * Math.cos(bearing * Math.PI / 180), h * Math.sin(bearing * Math.PI / 180)]
  return [(x+1)*xmax/2, (-y+1)*ymax/2]
}

function drawConnection({
  from,
  to,
  from_seconds,
  to_seconds,
  route_name,
  kind,
}, stops) {
  let [x1, y1] = stopCoords(stops[from], from_seconds)
  let [x2, y2] = stopCoords(stops[to], to_seconds)
  draw.line(x1, y1, x2, y2).attr({ class: route_name + ' ' + kind })
}

async function drawStops() {
  let response = await fetch("./example.json")
  let data = await response.json();

  for (let st of data.stops) {
    drawStop(st)
  }

  for (let connection of data.connections) {
    drawConnection(connection, data.stops)
  }
}

drawStops()
