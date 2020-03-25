import { SVG } from './node_modules/@svgdotjs/svg.js/dist/svg.esm.js'
var draw = SVG().addTo('body').size(1100, 1400)
const xmax = 1000, ymax = 1000
const maxSeconds = 1800

function drawStop(stop) {
  let {
    name,
  } = stop
  let [xd, yd] = stopCoords(stop)
  const stopDia = 6
  draw.circle(stopDia).move(xd - stopDia/2, yd - stopDia/2).fill('#0')
  draw.text(name).font({ size: 12 }).move(xd + stopDia + 5, yd - 5)
  draw.text
}

function stopCoords({
  bearing,
  seconds,
}) {
  let h = seconds / maxSeconds
  let [x, y] = [h * Math.cos(bearing * Math.PI / 180), h * Math.sin(bearing * Math.PI / 180)]
  return [(x+1)*xmax/2, (-y+1)*ymax/2]
}

function drawConnection({
  from,
  to,
  route_name,
  kind,
}, stops) {
  let [x1, y1] = stopCoords(stops[from])
  let [x2, y2] = stopCoords(stops[to])
  draw.line(x1, y1, x2, y2).stroke({ width: 1 }).stroke('black').attr({ class: route_name + ' ' + kind })
}

async function drawStops() {
  let response = await fetch("./example.json")
  let data = await response.json();
  // console.log(dagta.stops)

  for (let st of data.stops) {
    // console.log(stop)
    drawStop(st)
  }

  for (let connection of data.connections) {
    drawConnection(connection, data.stops)
  }
}

drawStops()
