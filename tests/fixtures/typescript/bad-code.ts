// TypeScript file with 'any' type issues

function process(x: any, y: any): any {
    return x + y;
}

function alsoBad(): any {
    return "result";
}

// Good code
function typed(a: string, b: number): string {
    return a + b;
}
