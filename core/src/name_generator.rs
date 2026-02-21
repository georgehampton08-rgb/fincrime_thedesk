//! Deterministic customer name generation using curated name lists.
//!
//! Provides realistic, diverse names for both personal and business customers.
//! All generation is deterministic (same RNG seed = same names).

use crate::rng::SubsystemRng;

/// Deterministic name generator using curated name lists
pub struct NameGenerator;

impl NameGenerator {
    /// Generate a full name (first + last) deterministically
    pub fn generate_full_name(rng: &mut SubsystemRng) -> String {
        let first_name = Self::generate_first_name(rng);
        let last_name = Self::generate_last_name(rng);
        format!("{} {}", first_name, last_name)
    }

    /// Generate first name from curated list
    pub fn generate_first_name(rng: &mut SubsystemRng) -> &'static str {
        let names = Self::first_names();
        let index = rng.next_u64_below(names.len() as u64) as usize;
        names[index]
    }

    /// Generate last name from curated list
    pub fn generate_last_name(rng: &mut SubsystemRng) -> &'static str {
        let names = Self::last_names();
        let index = rng.next_u64_below(names.len() as u64) as usize;
        names[index]
    }

    /// Generate business name for SmallBusiness/Commercial customers
    pub fn generate_business_name(rng: &mut SubsystemRng) -> String {
        let prefix = Self::business_prefixes();
        let suffix = Self::business_suffixes();
        let industry = Self::business_industries();

        let prefix_idx = rng.next_u64_below(prefix.len() as u64) as usize;
        let suffix_idx = rng.next_u64_below(suffix.len() as u64) as usize;
        let industry_idx = rng.next_u64_below(industry.len() as u64) as usize;

        // Format: "Prefix Industry Suffix" or "LastName Industry Suffix"
        if rng.chance(0.5) {
            format!("{} {} {}",
                prefix[prefix_idx],
                industry[industry_idx],
                suffix[suffix_idx])
        } else {
            format!("{} {} {}",
                Self::generate_last_name(rng),
                industry[industry_idx],
                suffix[suffix_idx])
        }
    }

    /// Curated list of 200 first names (diverse, realistic)
    fn first_names() -> &'static [&'static str] {
        &[
            // Male names (100)
            "James", "John", "Robert", "Michael", "William", "David", "Richard", "Joseph",
            "Thomas", "Charles", "Christopher", "Daniel", "Matthew", "Anthony", "Mark",
            "Donald", "Steven", "Paul", "Andrew", "Joshua", "Kenneth", "Kevin", "Brian",
            "George", "Timothy", "Ronald", "Edward", "Jason", "Jeffrey", "Ryan",
            "Jacob", "Gary", "Nicholas", "Eric", "Jonathan", "Stephen", "Larry", "Justin",
            "Scott", "Brandon", "Benjamin", "Samuel", "Raymond", "Gregory", "Frank",
            "Alexander", "Patrick", "Jack", "Dennis", "Jerry", "Tyler", "Aaron", "Jose",
            "Adam", "Nathan", "Henry", "Douglas", "Zachary", "Peter", "Kyle", "Noah",
            "Ethan", "Jeremy", "Walter", "Christian", "Keith", "Roger", "Terry", "Austin",
            "Sean", "Gerald", "Carl", "Harold", "Dylan", "Arthur", "Lawrence", "Jordan",
            "Jesse", "Bryan", "Billy", "Bruce", "Gabriel", "Juan", "Albert", "Willie",
            "Elijah", "Logan", "Joe", "Mason", "Roy", "Ralph", "Eugene", "Russell",
            "Bobby", "Victor", "Martin", "Ernest", "Phillip", "Todd", "Jesse", "Craig",

            // Female names (100)
            "Mary", "Patricia", "Jennifer", "Linda", "Barbara", "Elizabeth", "Susan",
            "Jessica", "Sarah", "Karen", "Lisa", "Nancy", "Betty", "Margaret", "Sandra",
            "Ashley", "Kimberly", "Emily", "Donna", "Michelle", "Carol", "Amanda", "Dorothy",
            "Melissa", "Deborah", "Stephanie", "Rebecca", "Sharon", "Laura", "Cynthia",
            "Kathleen", "Amy", "Angela", "Shirley", "Anna", "Brenda", "Pamela", "Emma",
            "Nicole", "Helen", "Samantha", "Katherine", "Christine", "Debra", "Rachel",
            "Carolyn", "Janet", "Catherine", "Maria", "Heather", "Diane", "Ruth", "Julie",
            "Olivia", "Joyce", "Virginia", "Victoria", "Kelly", "Lauren", "Christina",
            "Joan", "Evelyn", "Judith", "Megan", "Andrea", "Cheryl", "Hannah", "Jacqueline",
            "Martha", "Gloria", "Teresa", "Ann", "Sara", "Madison", "Frances", "Kathryn",
            "Janice", "Jean", "Abigail", "Alice", "Judy", "Sophia", "Grace", "Denise",
            "Amber", "Doris", "Marilyn", "Danielle", "Beverly", "Isabella", "Theresa",
            "Diana", "Natalie", "Brittany", "Charlotte", "Marie", "Kayla", "Alexis",
            "Lori", "Emma", "Ava", "Mia", "Sofia", "Ella"
        ]
    }

    /// Curated list of 200 last names (diverse, realistic)
    fn last_names() -> &'static [&'static str] {
        &[
            "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
            "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
            "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson",
            "White", "Harris", "Sanchez", "Clark", "Ramirez", "Lewis", "Robinson",
            "Walker", "Young", "Allen", "King", "Wright", "Scott", "Torres", "Nguyen",
            "Hill", "Flores", "Green", "Adams", "Nelson", "Baker", "Hall", "Rivera",
            "Campbell", "Mitchell", "Carter", "Roberts", "Gomez", "Phillips", "Evans",
            "Turner", "Diaz", "Parker", "Cruz", "Edwards", "Collins", "Reyes", "Stewart",
            "Morris", "Morales", "Murphy", "Cook", "Rogers", "Gutierrez", "Ortiz", "Morgan",
            "Cooper", "Peterson", "Bailey", "Reed", "Kelly", "Howard", "Ramos", "Kim",
            "Cox", "Ward", "Richardson", "Watson", "Brooks", "Chavez", "Wood", "James",
            "Bennett", "Gray", "Mendoza", "Ruiz", "Hughes", "Price", "Alvarez", "Castillo",
            "Sanders", "Patel", "Myers", "Long", "Ross", "Foster", "Jimenez", "Powell",
            "Jenkins", "Perry", "Russell", "Sullivan", "Bell", "Coleman", "Butler", "Henderson",
            "Barnes", "Gonzales", "Fisher", "Vasquez", "Simmons", "Romero", "Jordan", "Patterson",
            "Alexander", "Hamilton", "Graham", "Reynolds", "Griffin", "Wallace", "Moreno", "West",
            "Cole", "Hayes", "Bryant", "Herrera", "Gibson", "Ellis", "Tran", "Medina",
            "Aguilar", "Stevens", "Murray", "Ford", "Castro", "Marshall", "Owens", "Harrison",
            "Fernandez", "McDonald", "Woods", "Washington", "Kennedy", "Wells", "Vargas", "Henry",
            "Chen", "Freeman", "Webb", "Tucker", "Guzman", "Hawkins", "Crawford", "Olson",
            "Simpson", "Porter", "Hunter", "Gordon", "Mendez", "Silva", "Shaw", "Snyder",
            "Mason", "Dixon", "Munoz", "Hunt", "Hicks", "Holmes", "Palmer", "Wagner",
            "Black", "Robertson", "Boyd", "Rose", "Stone", "Salazar", "Fox", "Warren",
            "Mills", "Meyer", "Rice", "Schmidt", "Garza", "Daniels", "Hampton", "Nichols",
            "Stephens", "Soto", "Weaver", "Ryan", "Gardner", "Payne", "Grant", "Dunn"
        ]
    }

    /// Business name prefixes
    fn business_prefixes() -> &'static [&'static str] {
        &[
            "Premier", "Elite", "First", "Superior", "Quality", "Professional",
            "Advanced", "Reliable", "Trusted", "Expert", "Precision", "Metro",
            "City", "Valley", "Mountain", "Coastal", "Central", "United",
            "American", "National", "Global", "Universal", "Prime", "Best"
        ]
    }

    /// Business name suffixes
    fn business_suffixes() -> &'static [&'static str] {
        &[
            "LLC", "Inc", "Corp", "Co", "Group", "Associates", "Partners",
            "Solutions", "Services", "Enterprises", "Industries", "Holdings",
            "Ventures", "Consulting", "Systems", "Technologies", "Capital"
        ]
    }

    /// Business industries
    fn business_industries() -> &'static [&'static str] {
        &[
            "Construction", "Plumbing", "Electric", "HVAC", "Landscaping",
            "Consulting", "Marketing", "Design", "Development", "Accounting",
            "Legal", "Medical", "Dental", "Auto", "Retail", "Restaurant",
            "Cleaning", "Security", "Transportation", "Logistics", "Real Estate",
            "Insurance", "Financial", "Technology", "Manufacturing", "Wholesale"
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{RngBank, SubsystemSlot};

    #[test]
    fn name_generation_is_deterministic() {
        let mut rng_bank1 = RngBank::new(12345);
        let mut rng1 = rng_bank1.for_subsystem_at_tick(SubsystemSlot::Customer, 1);

        let name1 = NameGenerator::generate_full_name(&mut rng1);

        let mut rng_bank2 = RngBank::new(12345);
        let mut rng2 = rng_bank2.for_subsystem_at_tick(SubsystemSlot::Customer, 1);

        let name2 = NameGenerator::generate_full_name(&mut rng2);

        assert_eq!(name1, name2, "Same seed should produce same name");
    }

    #[test]
    fn generates_valid_full_names() {
        let mut rng_bank = RngBank::new(12345);
        let mut rng = rng_bank.for_subsystem_at_tick(SubsystemSlot::Customer, 1);

        for _ in 0..100 {
            let name = NameGenerator::generate_full_name(&mut rng);

            // Should have first and last name separated by space
            let parts: Vec<&str> = name.split_whitespace().collect();
            assert_eq!(parts.len(), 2, "Name should have exactly 2 parts: {}", name);

            // Should not be empty
            assert!(!parts[0].is_empty(), "First name should not be empty");
            assert!(!parts[1].is_empty(), "Last name should not be empty");
        }
    }

    #[test]
    fn generates_valid_business_names() {
        let mut rng_bank = RngBank::new(12345);
        let mut rng = rng_bank.for_subsystem_at_tick(SubsystemSlot::Customer, 1);

        for _ in 0..50 {
            let name = NameGenerator::generate_business_name(&mut rng);

            // Should have at least 2 words
            let parts: Vec<&str> = name.split_whitespace().collect();
            assert!(parts.len() >= 2, "Business name should have at least 2 parts: {}", name);

            // Should not be empty
            assert!(!name.is_empty(), "Business name should not be empty");
        }
    }
}
