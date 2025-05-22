type u64 = bigint;

interface Elm {
    readonly index: number;
    readonly id: u64;
}

type Pointer<T extends Elm> = [typeIndex: number, address: u64];

const enum EdgeType {
    ElmAccountFactors = "has_factor",
    ElmFactorSecures = "is_securing",
    FactorDelegates = "is_delegating",
    FactorSubordinates = "is_suborning",
}

const enum ElmType {
    Unk = 0,
    Map = 1,
    Vec = 2,
    Set = 3,
    Account = 1000,
    AccountFactorNormal = 1001,
    AccountFactorCrypto = 1002,
}

export class ElmUnk implements Elm {
    static readonly index: ElmType = ElmType.Unk;
    readonly index: ElmType = ElmUnk.index;
    readonly id: u64
    // other fields
    data: string
}

export class ElmMap implements Elm {
    static readonly index: ElmType = ElmType.Map;
    readonly index: ElmType = ElmMap.index;
    readonly id: u64
    // other fields
    length: u64
}

/*
export class ElmVec implements Elm {
    static readonly index: ElmType = ElmType.Vec;
    readonly index: ElmType = ElmVec.index;
    readonly id: u64
    // other fields
    length: u64
}
*/

export class ElmSet implements Elm {
    static readonly index: ElmType = ElmType.Set;
    readonly index: ElmType = ElmSet.index;
    readonly id: u64
    // other fields
    length: u64
}

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