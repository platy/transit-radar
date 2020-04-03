import React from "react";
import './Radar.css'
const xmax = 1000, ymax = 1000
const maxSeconds = 30 * 60

function Stop(stop) {
  let {
    name,
  } = stop
  let [cx, cy] = stopCoords(stop)
  const stopR = 3
  return <>
    <circle r={stopR} cx={cx} cy={cy} />
    <text x={cx + stopR + 6} y={cy + 4}>
      {name}
    </text>
  </>
}

function stopCoords({
  bearing,
  seconds,
}) {
  let h = seconds / maxSeconds
  let [x, y] = [h * Math.cos(bearing * Math.PI / 180), h * Math.sin(bearing * Math.PI / 180)]
  return [(x+1)*xmax/2, (-y+1)*ymax/2]
}

function controlPoints([x1, y1], [x2, y2], [x3, y3]) {
  let cpfrac = 0.3
  let angle_to_prev = Math.atan2(y2 - y1, x2 - x1)
  let angle_to_next = Math.atan2(y2 - y3, x2 - x3)
  // things to adjust and improve
  let angle_to_tangent = (Math.PI + angle_to_next + angle_to_prev) / 2
  let cp2mag = -cpfrac * Math.sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1))
  let cp3mag = cpfrac * Math.sqrt((x2 - x3) * (x2 - x3) + (y2 - y3) * (y2 - y3))
  // ^^ improve these
  let dx = Math.cos(angle_to_tangent)
  let dy = Math.sin(angle_to_tangent)
  if (angle_to_prev < angle_to_next) {
    dy *= -1
    dx *= -1
  }
  return [[x2 + dx * cp2mag, y2 + dy * cp2mag], [x2 + dx * cp3mag, y2 + dy * cp3mag]]
}

// Find a control point that directs the curve away from the origin, [x1, y1] is used if that works, otherwise orthogonal to the origin
function initialControlPoint([x1, y1], [x2, y2], [ox, oy]) {
  let angle_to_origin = Math.atan2(oy - y1, ox - x1)
  let angle_to_next = Math.atan2(y2 - y1, x2 - x1)
  // things to adjust and improve
  let angle_between = angle_to_origin - angle_to_next
  if (angle_between < 0) angle_between = 2 * Math.PI + angle_between
  if (angle_between < Math.PI / 2) {
    let cpangle = angle_to_origin - Math.PI / 2
    let cpmag = 0.5 * Math.sqrt((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2))
    return [x1 + cpmag * Math.cos(cpangle), y1 + cpmag * Math.sin(cpangle)]
  } else if (angle_between > 3 * Math.PI / 2) {
    let cpangle = angle_to_origin + Math.PI / 2
    let cpmag = 0.5 * Math.sqrt((x1 - x2) * (x1 - x2) + (y1 - y2) * (y1 - y2))
    return [x1 + cpmag * Math.cos(cpangle), y1 + cpmag * Math.sin(cpangle)]
  } else {
    return [x1, y1]
  }
}

function Route({
  route_name,
  kind,
  segments,
}, stops, origin) {
  let prevx, prevy
  let points = []
  for (let {
    from,
    to,
    from_seconds,
    to_seconds,
  } of segments) {
    let [x1, y1] = stopCoords({ bearing: stops[from].bearing, seconds: from_seconds })
    let [x2, y2] = stopCoords({ bearing: stops[to].bearing, seconds: to_seconds })
    if (x1 !== prevx || y1 !== prevy) {
      points.push({x: x1, y: y1, move: true})
    }
    points.push({x: x2, y: y2})
    prevx = x2
    prevy = y2
  }
  let path = ''
  for (var i=0; i< points.length; i++) {
    let {x, y, cpbx, cpby, move} = points[i] // TODO support move
    if (i === 0) { // must be move
      let [cpbx, cpby] = initialControlPoint([x, y], [points[1].x, points[1].y], origin)
      path += `M ${x} ${y} `
      points[i+1].cpbx = cpbx
      points[i+1].cpby = cpby
    } else if (i < points.length - 1) {
      let [[cpex, cpey], [cpbx2, cpby2]] = controlPoints([points[i-1].x, points[i-1].y], [x, y], [points[i+1].x, points[i+1].y])
      points[i+1].cpbx = cpbx2
      points[i+1].cpby = cpby2
      if (move) {
        path += `M ${x} ${y} `
      } else {
        path += `C ${cpbx} ${cpby}, ${cpex} ${cpey}, ${x} ${y} `
      }
    } else if (!move) { // don't draw a move
      path += `C ${cpbx} ${cpby}, ${x} ${y}, ${x} ${y}`
    }
  }
  return <path d={path} class={route_name + ' ' + kind} />
}

function Connection({
  from,
  to,
  from_seconds,
  to_seconds,
  route_name,
}, stops) {
  let [x1, y1] = stopCoords({ bearing: stops[from].bearing, seconds: from_seconds })
  let [x2, y2] = stopCoords({ bearing: stops[to].bearing, seconds: to_seconds })
  let className
  if (route_name) {
    className = route_name + ' Connection'
  } else {
    className = 'Transfer'
  }
  return <line x1={x1} y1={y1} x2={x2} y2={y2} class={className} />
}

export default function Radar({data}) {
  if (!data) return <p>No data</p>
  // direct the origin stop to the left instead of the right to avoid running over its label
  let origin = data.stops[0]
  if (origin.bearing === 0 && origin.seconds === 0) {
    origin.bearing = 180
  }

  return <svg xmlns="http://www.w3.org/2000/svg" width={1100} height={1000}>
    <circle class="grid" r={(10 * 60 / maxSeconds) * xmax / 2} cx={500} cy={500} />
    <circle class="grid" r={(20 * 60 / maxSeconds) * xmax / 2} cx={500} cy={500} />
    <circle class="grid" r={(30 * 60 / maxSeconds) * xmax / 2} cx={500} cy={500} />
    {data.stops.map(Stop)}
    {data.connections.reverse().map(conn => Connection(conn, data.stops))}
    {data.trips.map(trip => Route(trip, data.stops, [500, 500]))}
  </svg>
}
