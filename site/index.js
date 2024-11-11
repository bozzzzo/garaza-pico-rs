document.onreadystatechange = () => {
    if (document.readyState === "complete") {
        initApplication();
    }
};

const button_id = "button";

initApplication = () => {
    setTimeout(() => {
        let b = document.getElementById(button_id);
        b.setAttribute("class", "scripted");
        b.onclick = async() => { await press_me(); };
    }, 1000)
}

msg = (data) => {
    let p = document.createElement("p");
    p.appendChild(document.createTextNode(""+data));
    let b = document.getElementById(button_id);
    b.after(p);
}

press_me = async () => {
    msg("butn butn butn");
    let data = await (
        async()=> {
            let response = await fetch("api/butn", {method: "POST"});
            return await response.text();
        })()
        .catch((error) => {return error;});
    msg(data);
}
