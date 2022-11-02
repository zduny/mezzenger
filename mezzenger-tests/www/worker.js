importScripts("worker_wasm.js");

async function initWorker() {
    await wasm_bindgen("worker_wasm_bg.wasm");
};

console.log("Worker: initializing worker...");
initWorker();
