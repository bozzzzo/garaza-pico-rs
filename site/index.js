document.onreadystatechange = () => {
    if (document.readyState === "complete") {
        initApplication();
    }
};

initApplication = () => {
    setTimeout(() => {
    document.getElementById("paragraph").setAttribute("class", "scripted");
    }, 2000)
}
