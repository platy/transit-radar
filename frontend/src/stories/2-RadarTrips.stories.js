import React from 'react';
import Radar from '../Radar';
import voltaData from './volta-data.json'

export default {
  title: 'Radar Trips',
  component: Radar,
};

const schönefeldData = {"stops":[{"bearing":0,"name":"S Flughafen Berlin-Schönefeld Bhf","seconds":0},{"bearing":-14.405517602496182,"name":"Flughafen BER - Terminal 5 [Bus] (ab 31.10.2020)","seconds":300},{"bearing":15.97752956392164,"name":"S Grünbergallee","seconds":468},{"bearing":-15.797810417176244,"name":"Flughafen Schönefeld Terminal A-D (Airport)","seconds":540},{"bearing":-7.614610642490123,"name":"Schönefeld, Flughafen Parkplatz P1","seconds":540},{"bearing":19.7285909323547,"name":"S Altglienicke","seconds":612},{"bearing":13.65377676791022,"name":"Berlin, Am Seegraben","seconds":828},{"bearing":57.8886645386349,"name":"S Adlershof","seconds":876},{"bearing":16.121590244615195,"name":"Berlin, Johannes-Tobei-Str.","seconds":972},{"bearing":79.05378365638792,"name":"S Betriebsbahnhof Schöneweide","seconds":1032},{"bearing":56.27041399588503,"name":"S Adlershof (Bln) [Bus Dörpfeldstr. n. Köpenick]","seconds":1116},{"bearing":55.65637590683756,"name":"S Adlershof (Bln) [Bus Dörpfeldstr. v. Köpenick]","seconds":1176},{"bearing":19.40448903487016,"name":"S Grünau","seconds":1188},{"bearing":93.1039153000253,"name":"S Schöneweide Bhf","seconds":1200},{"bearing":55.37597835848228,"name":"Berlin, Abtstr.","seconds":1236},{"bearing":107.82170613592226,"name":"S Baumschulenweg","seconds":1362},{"bearing":20.330048868422875,"name":"S Grünau [Bruno-Taut-Str.]","seconds":1368},{"bearing":19.457853495831706,"name":"S Grünau [Richterstr.]","seconds":1368},{"bearing":19.427073147038033,"name":"S Grünau [Adlergestell]","seconds":1428},{"bearing":20.19131414243797,"name":"S Grünau [Wassersportallee]","seconds":1428},{"bearing":93.2830910851194,"name":"S Schöneweide/Sterndamm","seconds":1500},{"bearing":-10.160907325124485,"name":"S Eichwalde","seconds":1506},{"bearing":121.1008202129552,"name":"S Köllnische Heide","seconds":1518},{"bearing":91.52650120413172,"name":"S Schöneweide [Vorplatz]","seconds":1560},{"bearing":109.97495082711288,"name":"Berlin, Kiefholzstr./Baumschulenstr.","seconds":1662},{"bearing":-19.354037515399966,"name":"S Zeuthen","seconds":1698}],"connections":[{"from_seconds":0,"to_seconds":0,"from":0,"to":0,"route_name":null},{"from_seconds":0,"to_seconds":300,"from":0,"to":1,"route_name":null},{"from_seconds":0,"to_seconds":318,"from":0,"to":0,"route_name":"S45"},{"from_seconds":0,"to_seconds":540,"from":0,"to":3,"route_name":null},{"from_seconds":0,"to_seconds":540,"from":0,"to":4,"route_name":null},{"from_seconds":468,"to_seconds":828,"from":2,"to":6,"route_name":null},{"from_seconds":612,"to_seconds":972,"from":5,"to":8,"route_name":null},{"from_seconds":876,"to_seconds":1116,"from":7,"to":10,"route_name":null},{"from_seconds":876,"to_seconds":1176,"from":7,"to":11,"route_name":null},{"from_seconds":876,"to_seconds":984,"from":7,"to":7,"route_name":"S46"},{"from_seconds":876,"to_seconds":1236,"from":7,"to":14,"route_name":null},{"from_seconds":1188,"to_seconds":1368,"from":12,"to":16,"route_name":null},{"from_seconds":1188,"to_seconds":1368,"from":12,"to":17,"route_name":null},{"from_seconds":1188,"to_seconds":1428,"from":12,"to":18,"route_name":null},{"from_seconds":1188,"to_seconds":1428,"from":12,"to":19,"route_name":null},{"from_seconds":1200,"to_seconds":1500,"from":13,"to":20,"route_name":null},{"from_seconds":1200,"to_seconds":1560,"from":13,"to":23,"route_name":null},{"from_seconds":1362,"to_seconds":1662,"from":15,"to":24,"route_name":null}],"trips":[{"route_name":"S45","kind":"SuburbanRailway","segments":[{"from_seconds":318,"to_seconds":468,"from":0,"to":2},{"from_seconds":468,"to_seconds":612,"from":2,"to":5},{"from_seconds":612,"to_seconds":876,"from":5,"to":7},{"from_seconds":876,"to_seconds":1032,"from":7,"to":9},{"from_seconds":1032,"to_seconds":1200,"from":9,"to":13},{"from_seconds":1200,"to_seconds":1362,"from":13,"to":15},{"from_seconds":1362,"to_seconds":1518,"from":15,"to":22}]},{"route_name":"S46","kind":"SuburbanRailway","segments":[{"from_seconds":984,"to_seconds":1188,"from":7,"to":12},{"from_seconds":1188,"to_seconds":1506,"from":12,"to":21},{"from_seconds":1506,"to_seconds":1698,"from":21,"to":25}]}]}

export const Schoenefeld = () => <Radar data={schönefeldData} />;
export const Voltastraße = () => <Radar data={voltaData} />;
