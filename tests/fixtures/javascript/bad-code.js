// This file should trigger multiple issues

console.log("This should be flagged as warning");

function dangerousFunction() {
    eval("This should be flagged as error");
    console.log("Another console.log");
}

function alsoBad() {
    console.log("Yet another");
    eval("Another eval");
}
