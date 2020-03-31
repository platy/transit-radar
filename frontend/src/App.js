import React, { useState, useEffect } from "react";
import './App.css';
import Radar from './Radar';
import Autocomplete from 'react-autocomplete'

function App() {
  const [suggestions, setSuggestions] = useState([]);
  const [query, setQuery] = useState('');
  const [station, setStation] = useState(null);
  const [radarData, setRadarData] = useState({});

  useEffect(() => {
    let ignore = false;

    async function fetchData() {
      if (!ignore && query.length > 3) {
        const result = await fetch('http://localhost:8080/searchStation/' + query);
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
        const result = await fetch('http://localhost:8080/from/' + station);
        const json = await result.json();
        setRadarData(json);
        Radar(json)
      }
    }

    fetchData();
    return () => { ignore = true; }
  }, [station]);

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
    </>
  );
}


export default App;
