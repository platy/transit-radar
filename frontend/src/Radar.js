import './Radar.css'
import { SVG } from '@svgdotjs/svg.js'
const xmax = 1000, ymax = 1000
const maxSeconds = 30 * 60
var draw

function drawStop(stop) {
  let {
    name,
  } = stop
  let [cx, cy] = stopCoords(stop)
  const stopDia = 6
  draw.circle(stopDia).attr({ cx, cy })
  draw.text(name).move(cx + stopDia + 2, cy - 6)
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
  from_seconds,
  to_seconds,
  route_name,
  kind,
}, stops) {
  let [x1, y1] = stopCoords({ bearing: stops[from].bearing, seconds: from_seconds })
  let [x2, y2] = stopCoords({ bearing: stops[to].bearing, seconds: to_seconds })
  draw.line(x1, y1, x2, y2).attr({ class: route_name + ' ' + kind })
}

export default async function Radar(data) {
  if (document.querySelector('svg'))
    document.querySelector('svg').remove()

  draw = SVG().addTo('body').size(1100, 1400)
  draw.circle((10 * 60 / maxSeconds) * xmax).attr({ cx: xmax / 2, cy: ymax / 2, class: 'grid'})
  draw.circle((20 * 60 / maxSeconds) * xmax).attr({ cx: xmax / 2, cy: ymax / 2, class: 'grid'})
  draw.circle((30 * 60 / maxSeconds) * xmax).attr({ cx: xmax / 2, cy: ymax / 2, class: 'grid'})

  // direct the origin stop to the left instead of the right to avoid running over its label
  let origin = data.stops[0]
  if (origin.bearing === 0 && origin.seconds === 0) {
    origin.bearing = 180
  }

  for (let st of data.stops) {
    drawStop(st)
  }

  for (let connection of data.connections.reverse()) {
    drawConnection(connection, data.stops)
  }
}