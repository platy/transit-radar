import React, { useState, useEffect } from "react";
import './App.css';
import Radar from './Radar';
import Autocomplete from 'react-autocomplete'

const queryBaseUrl = process.env.REACT_APP_BACKEND_URL || '';

function App() {
  const [suggestions, setSuggestions] = useState([]);
  const [query, setQuery] = useState('');
  const [station, setStation] = useState(null);
  const [radarData, setRadarData] = useState(null);
  const [showStations, setShowStations] = useState(true);
  const [animate, setAnimate] = useState(true);
  const [showSBahn, setShowSBahn] = useState(true);
  const [showUBahn, setShowUBahn] = useState(true);
  const [showBus, setShowBus] = useState(false);
  const [showTrams, setShowTrams] = useState(false);
  const [showRegional, setShowRegional] = useState(false);

  async function fetchRadarData() {
    const result = await fetch(`${queryBaseUrl}/from/${station}?ubahn=${showUBahn}&sbahn=${showSBahn}&bus=${showBus}&tram=${showTrams}&regio=${showRegional}`);
    const json = await result.json();
    setRadarData(json)
  }

  useEffect(() => {
    let ignore = false;

    async function fetchData() {
      if (!ignore && query.length > 3) {
        const result = await fetch(queryBaseUrl + '/searchStation/' + query);
        const json = await result.json();
        setSuggestions(json);
      }
    }

    fetchData();
    return () => { ignore = true; }
  }, [query]);

  useEffect(() => {
    let ignore = false;

    async function fetchData() {
      if (!ignore && station) {
        fetchRadarData()
      }
    }

    fetchData();
    return () => { ignore = true; }
  }, [station]);

  useEffect(() => {
    let timeout
    let ignore = false

    async function tick() {
      await fetchRadarData()
      if (!ignore) timeout = setTimeout(tick, 1000)
    }
    if (animate && station) tick()
    return () => { clearTimeout(timeout); ignore = true }
  }, [animate, station, showUBahn, showSBahn, showBus, showTrams, showRegional]);

  return (
    <>
      <span>Search a station in Berlin :</span>
      <Autocomplete
        getItemValue={(item) => item.name}
        items={suggestions}
        renderItem={(item, isHighlighted) =>
          <div style={{ background: isHighlighted ? 'lightgray' : 'white' }}>
            {item.name}
          </div>
        }
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onSelect={(val) => setStation(val)}
      />
      <h2>{station}</h2>
      <input type="button" onClick={() => fetchRadarData()} value="Reload" />
      <input type="checkbox" 
        onChange={(e) => setShowStations(e.target.checked)}
        checked={showStations} 
        name="showStations" />
      <label for="showStations">Show Stations</label>
      <input type="checkbox" 
        onChange={(e) => setAnimate(e.target.checked)}
        checked={animate} 
        name="animate" />
      <label for="animate">Animate</label>
      <input type="checkbox" 
        onChange={(e) => setShowSBahn(e.target.checked)}
        checked={showSBahn} 
        name="animate" />
      <label for="animate">Show SBahn</label>
      <input type="checkbox" 
        onChange={(e) => setShowUBahn(e.target.checked)}
        checked={showUBahn} 
        name="animate" />
      <label for="animate">Show UBahn</label>
      <input type="checkbox" 
        onChange={(e) => setShowBus(e.target.checked)}
        checked={showBus} 
        name="animate" />
      <label for="animate">Show buses</label>
      <input type="checkbox" 
        onChange={(e) => setShowTrams(e.target.checked)}
        checked={showTrams} 
        name="animate" />
      <label for="animate">Show trams</label>
      <input type="checkbox" 
        onChange={(e) => setShowRegional(e.target.checked)}
        checked={showRegional} 
        name="animate" />
      <label for="animate">Show regional trains</label>
      <Radar data={radarData} showStations={showStations} />
    </>
  );
}


export default App;
