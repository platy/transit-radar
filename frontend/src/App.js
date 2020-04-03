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
        const result = await fetch(queryBaseUrl + '/from/' + station);
        const json = await result.json();
        setRadarData(json)
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
      <p>
        The transit radar shows all the destinations you could reach within 30mins
        using SBahn or UBahn from the selected station, it currently assumes you are departing 
        on a Friday at 19:00 and uses VBB's published timetables at 24/03/2020.
      </p>
      <Radar data={radarData} />
    </>
  );
}


export default App;
