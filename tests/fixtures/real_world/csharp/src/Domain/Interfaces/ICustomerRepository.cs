using Commerce.Domain.Entities;

namespace Commerce.Domain.Interfaces;

public interface ICustomerRepository
{
    Customer? GetById(string id);
    void Save(Customer customer);
    IEnumerable<Customer> GetAllActive();
}
