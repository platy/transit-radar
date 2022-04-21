function setupAutosearch() {
    const searchbox = document.getElementsByName('q')[0];

    const main = document.getElementsByTagName('main')[0];
    history.replaceState({term: searchbox.value, results: main.outerHTML}, '');

    let isFetching = false;
    let hasPushedState = false;
    
    searchbox.oninput = (event) => {
        if (!isFetching) {
            isFetching = true;
            const term = event.target.value;
            const query = new URLSearchParams({'q': term});
            const req = new Request('./auto?' + query.toString());
            fetch(req).then((resp) => resp.text()).then((results) => {
                isFetching = false;
                if (hasPushedState) {
                    history.replaceState({term: term, results: results}, '', '.?' + query.toString());
                } else {
                    history.pushState({term: term, results: results}, '', '.?' + query.toString());
                    hasPushedState = true;
                }
                const main = document.getElementsByTagName('main')[0];
                main.outerHTML = results
            }, (reason) => {
                isFetching = false;
                console.error('fetch rejected', reason);
            })
        }
    };

    window.onpopstate = ({state: {term, results}}) => {
        searchbox.value = term;
        const main = document.getElementsByTagName('main')[0];
        main.outerHTML = results
    };
}

setupAutosearch();
