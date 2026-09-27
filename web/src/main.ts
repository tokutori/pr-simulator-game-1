const statusElement = document.getElementById("status");
if (statusElement === null) {
  throw new Error("Required status element is missing");
}
statusElement.textContent = "フライト機能は準備中である。";
