export class ElmAccount implements Elm {
    static readonly index: ElmType = ElmType.Account;
    readonly index: ElmType = ElmAccount.index;
    readonly id: u64
    // other fields
}

export class ElmAccountFactorNormal implements Elm {
    static readonly index: ElmType = ElmType.AccountFactorNormal;
    readonly index: ElmType = ElmAccountFactorNormal.index;
    readonly id: u64
    // other fields
    username: string;
    passwordHash: string;
    passwordSalt: string;
}

export class ElmAccountFactorCrypto implements Elm {
    static readonly index: ElmType = ElmType.AccountFactorCrypto;
    readonly index: ElmType = ElmAccountFactorCrypto.index;
    readonly id: u64
    // other fields  
    scheme: string;
    public_key: string;
    private_key_locked: string;
}