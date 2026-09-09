for (const button of document.querySelectorAll("button[data-url]")) {
  button.addEventListener("click", async () => {
    const result = document.querySelector("#result");
    result.textContent = `Calling ${button.dataset.url}...`;
    try {
      const response = await fetch(button.dataset.url);
      result.textContent = `${response.status} ${await response.text()}`;
    } catch (error) {
      result.textContent = `Request failed: ${error}`;
    }
  });
}
