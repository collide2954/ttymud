use crate::random::Random;

const INITIALS: &[u8] = b"abcdefghjklmnprstvwz";
const SURNAMES: [&str; 16] = [
    "ortiz", "lin", "okafor", "novak", "silva", "haddad", "kowal", "tanaka", "mbeki", "larsen",
    "costa", "ivanova", "reyes", "osei", "becker", "nakamura",
];

pub(crate) fn person(random: &mut Random) -> String {
    let initial = char::from(INITIALS[random.below(INITIALS.len())]);
    format!("{initial}.{}", SURNAMES[random.below(SURNAMES.len())])
}
