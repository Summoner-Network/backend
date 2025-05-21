type u64 = bigint;

interface Elm {
    readonly index: number;
    readonly id: u64;
}

type Pointer<T extends Elm> = [typeIndex: number, address: u64];

const enum EdgeType {
    ElmAccountFactors = 0,
    ElmFactorSecures = 1
}

const enum ElmType {
    Account = 1,
    AccountFactorNormal = 2,
    AccountFactorCrypto = 3,
    
    FactorDelegates = 4,
    FactorSubordinates = 5,
}

/** TODO: Create an Elm from these
  type: number;
  id: bigint;
  attributes: { [key: string]: string };
*/

function createElm(type: number, id: u64, attributes: { [key: string]: string }): Elm {
    return {
        index: type,
        id,
        ...attributes,
    };
}

function loadElm<T extends Elm>(type: number, id: u64, attributes: { [key: string]: string }): T {
    const elm = createElm(type, id, attributes);
    if (!('index' in elm) || !('id' in elm)) {
        throw new Error('Invalid Elm object');
    }
    return elm as T;
}